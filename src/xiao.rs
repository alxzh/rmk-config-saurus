//! Seeed XIAO nRF52840-specific power and battery support.
//!
//! RMK's generated nRF battery ADC currently uses the SAADC defaults: a 10 us
//! acquisition time and no oversampling. The XIAO's 1 MOhm / 510 kOhm battery
//! divider has a Thevenin resistance of about 338 kOhm, for which Nordic
//! specifies at least 20 us. We use 40 us and 16x hardware oversampling to give
//! the sample-and-hold capacitor enough settling time and reduce noise.

use embassy_nrf::gpio::{Level, Output, OutputDrive};
use embassy_nrf::interrupt::{self, InterruptExt};
use embassy_nrf::peripherals::{
    P0_14, P0_20, P0_21, P0_22, P0_23, P0_24, P0_25, P0_31, QSPI, SAADC,
};
use embassy_nrf::qspi::{self, DeepPowerDownConfig, Qspi};
use embassy_nrf::saadc::{self, Input as _, Saadc};
use embassy_nrf::{Peri, bind_interrupts};
use rmk::core_traits::Runnable;
use rmk::event::{BatteryAdcEvent, EventSubscriber, publish_event};
use rmk::processor::Processor;

/// XIAO battery divider values: VBAT -- 1 MOhm -- ADC -- 510 kOhm -- GND.
pub const ADC_DIVIDER_MEASURED: u32 = 510;
pub const ADC_DIVIDER_TOTAL: u32 = 1510;

const BATTERY_POLL_SECONDS: u64 = 30;
const EXTERNAL_FLASH_CAPACITY: u32 = 2 * 1024 * 1024;
// Set false for recovery testing if a future XIAO hardware revision uses a
// different external flash or its boot flow is incompatible with DPM.
const EXTERNAL_FLASH_DEEP_POWER_DOWN: bool = true;

bind_interrupts!(struct XiaoIrqs {
    SAADC => saadc::InterruptHandler;
    QSPI => qspi::InterruptHandler<QSPI>;
});

/// Battery ADC task for the original Seeed Studio XIAO nRF52840.
pub struct XiaoBatteryAdc {
    adc: Saadc<'static, 1>,
    sample: [i16; 1],
}

impl XiaoBatteryAdc {
    /// Put the unused onboard QSPI flash into deep power-down, then initialize
    /// the high-impedance battery ADC.
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        saadc_peripheral: Peri<'static, SAADC>,
        battery_enable: Peri<'static, P0_14>,
        battery_pin: Peri<'static, P0_31>,
        qspi_peripheral: Peri<'static, QSPI>,
        qspi_sck: Peri<'static, P0_21>,
        qspi_csn: Peri<'static, P0_25>,
        qspi_io0: Peri<'static, P0_20>,
        qspi_io1: Peri<'static, P0_24>,
        qspi_io2: Peri<'static, P0_22>,
        qspi_io3: Peri<'static, P0_23>,
    ) -> Self {
        // Seeed requires READ_BAT_ENABLE (P0.14) to be held low before P0.31
        // is used. Persist it here rather than relying on RMK's generated
        // output ordering, which differs between central and peripheral.
        Output::new(battery_enable, Level::Low, OutputDrive::Standard).persist();

        power_down_external_flash(
            qspi_peripheral,
            qspi_sck,
            qspi_csn,
            qspi_io0,
            qspi_io1,
            qspi_io2,
            qspi_io3,
        );

        let mut channel = saadc::ChannelConfig::single_ended(battery_pin.degrade_saadc());
        channel.time = saadc::Time::_40US;

        let mut config = saadc::Config::default();
        config.oversample = saadc::Oversample::Over16x;

        interrupt::SAADC.set_priority(interrupt::Priority::P3);
        let adc = Saadc::new(saadc_peripheral, XiaoIrqs, config, [channel]);
        adc.calibrate().await;

        Self { adc, sample: [0] }
    }

    async fn run_adc(&mut self) -> ! {
        // Give RMK's BatteryProcessor time to create its event subscription,
        // then report at the same 30 second interval used by RMK's generated
        // battery ADC task.
        embassy_time::Timer::after_millis(100).await;
        loop {
            self.adc.sample(&mut self.sample).await;
            let raw = self.sample[0].max(0) as u16;
            let millivolts =
                raw as u64 * 3600 * ADC_DIVIDER_TOTAL as u64 / (4096 * ADC_DIVIDER_MEASURED as u64);

            defmt::debug!("Battery ADC: raw={}, approx={}mV", raw, millivolts);
            publish_event(BatteryAdcEvent(raw));

            embassy_time::Timer::after_secs(BATTERY_POLL_SECONDS).await;
        }
    }
}

/// `#[register_processor(event)]` starts processors through `process_loop`.
/// This task is a producer rather than an event consumer, so its loop is
/// overridden and its subscriber is deliberately never constructed at runtime.
impl Processor for XiaoBatteryAdc {
    type Event = BatteryAdcEvent;

    fn subscriber() -> impl EventSubscriber<Event = Self::Event> {
        NeverSubscriber
    }

    async fn process(&mut self, _event: Self::Event) {}

    async fn process_loop(&mut self) -> ! {
        self.run_adc().await
    }
}

impl Runnable for XiaoBatteryAdc {
    async fn run(&mut self) -> ! {
        self.run_adc().await
    }
}

struct NeverSubscriber;

impl EventSubscriber for NeverSubscriber {
    type Event = BatteryAdcEvent;

    async fn next_event(&mut self) -> Self::Event {
        core::future::pending().await
    }
}

/// Enter deep power-down on the XIAO's unused Puya P25Q16H external flash.
///
/// Embassy's QSPI `Drop` implementation requests DPM, waits for entry, leaves
/// CS high, deactivates QSPI, and disconnects the remaining pins. The flash
/// specifies 3 us maximum entry and 8 us maximum exit times; one 16 us unit is
/// sufficient for each.
fn power_down_external_flash(
    qspi_peripheral: Peri<'static, QSPI>,
    sck: Peri<'static, P0_21>,
    csn: Peri<'static, P0_25>,
    io0: Peri<'static, P0_20>,
    io1: Peri<'static, P0_24>,
    io2: Peri<'static, P0_22>,
    io3: Peri<'static, P0_23>,
) {
    if !EXTERNAL_FLASH_DEEP_POWER_DOWN {
        defmt::warn!("XIAO external flash deep power-down is disabled");
        return;
    }

    let mut config = qspi::Config::default();
    config.capacity = EXTERNAL_FLASH_CAPACITY;
    config.deep_power_down = Some(DeepPowerDownConfig {
        enter_time: 1,
        exit_time: 1,
    });

    let flash = Qspi::new(
        qspi_peripheral,
        XiaoIrqs,
        sck,
        csn,
        io0,
        io1,
        io2,
        io3,
        config,
    );

    // Dropping is intentional: it is what enters DPM and powers down QSPI.
    drop(flash);
    defmt::debug!("XIAO external flash entered deep power-down");
}
