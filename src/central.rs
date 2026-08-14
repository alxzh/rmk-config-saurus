#![no_main]
#![no_std]

mod xiao;

use rmk::macros::rmk_central;

#[rmk_central]
mod keyboard_central {
    // Register the consumer before the ADC producer so its subscription exists
    // before the first immediate battery sample is published.
    #[register_processor(event)]
    fn battery_processor() -> rmk::input_device::battery::BatteryProcessor {
        rmk::input_device::battery::BatteryProcessor::new(
            crate::xiao::ADC_DIVIDER_MEASURED,
            crate::xiao::ADC_DIVIDER_TOTAL,
        )
    }

    #[register_processor(event)]
    async fn xiao_battery_adc() -> crate::xiao::XiaoBatteryAdc {
        crate::xiao::XiaoBatteryAdc::new(
            p.SAADC, p.P0_14, p.P0_31, p.QSPI, p.P0_21, p.P0_25, p.P0_20, p.P0_24, p.P0_22, p.P0_23,
        )
        .await
    }
}
