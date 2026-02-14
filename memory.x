MEMORY
{
  /* NOTE 1 K = 1 KiB = 1024 bytes */
  /* These values correspond to the nRF52840 WITH Adafruit nRF52 bootloader */
  /* (used by Seeed XIAO BLE and nice!nano) */
  FLASH : ORIGIN = 0x00001000, LENGTH = 1020K
  RAM : ORIGIN = 0x20000008, LENGTH = 255K
}
