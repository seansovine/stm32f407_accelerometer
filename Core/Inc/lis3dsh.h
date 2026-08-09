#ifndef LIS3DSH_H_
#define LIS3DSH_H_

#include <stm32f4xx_hal.h>

#include <stdint.h>

// Global sensor state variables.

extern uint8_t x[2];
extern uint8_t y[2];
extern uint8_t z[2];

// Functions to interact with LIS sensor over SPI.

HAL_StatusTypeDef LIS_Read_data(uint8_t addr, uint8_t *data, uint16_t size);

HAL_StatusTypeDef LIS_Init(SPI_HandleTypeDef inHspi1);

HAL_StatusTypeDef LIS_Read();

// These assume USB OTG peripheral has been configured for VTC
// and that RGBO LEDs have been configured to GPIOs PD12-15.

HAL_StatusTypeDef LIS_Check_Status_USB();

uint8_t LIS_Debug_Log_USB();

#endif // LIS3DSH_H_
