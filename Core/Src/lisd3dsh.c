#include <lis3dsh.h>

#include <usbd_cdc_if.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

// Register addrs for LIS3DSH accelerometer.

#define WHO_AM_I   (0x0F)
#define CTRL_REG4  (0x20)
#define STATUS_REG (0x18)
#define OUT_X_LOW  (0x28)
#define OUT_X_HIGH (0x29)
#define OUT_Y_LOW  (0x2A)
#define OUT_Y_HIGH (0x2B)
#define OUT_Z_LOW  (0x2C)
#define OUT_Z_HIGH (0x2D)

// Control reg 4 init values.

#define LIS_CR4_INIT  0x47
#define WAIT_FOR_READ 0x08

// Timeout before and after sending USB VTC msg.
//
// Explanation: We need as small delay between consecutive transmits
// on the USB VTG peripheral, because they are buffered and sent
// asynchronously and without the delay the second write encounters
// a busy device and fails.

#define VTC_TIMEOUT 5

// Vars for accelerometer readings:

#define CANARY_INIT {0xDE, 0xAD}

// Global sensor state variables.

uint8_t x[2] = CANARY_INIT;
uint8_t y[2] = CANARY_INIT;
uint8_t z[2] = CANARY_INIT;

// SPI handle from main application.

static SPI_HandleTypeDef hspi1;

// Sensor interaction functions.

#define LIS_HANDLE_ERROR                                                                                               \
  do                                                                                                                   \
  {                                                                                                                    \
    return HAL_ERROR;                                                                                                  \
  } while (0)

HAL_StatusTypeDef SPI_Transmit_Byte(uint8_t byte)
{
  return HAL_SPI_Transmit(&hspi1, &byte, 1, HAL_MAX_DELAY);
}

// NOTE: PE3 is the chip select for the LSI SPI interface. We have
// to toggle it manually because it isn't hardwired on this board.

HAL_StatusTypeDef LIS_Init(SPI_HandleTypeDef inHspi1)
{
  hspi1                    = inHspi1;
  HAL_StatusTypeDef result = HAL_OK;

  HAL_GPIO_WritePin(GPIOE, GPIO_PIN_3, GPIO_PIN_RESET);
  if (SPI_Transmit_Byte(CTRL_REG4) || SPI_Transmit_Byte(LIS_CR4_INIT | WAIT_FOR_READ))
  {
    result = HAL_ERROR;
  }
  HAL_GPIO_WritePin(GPIOE, GPIO_PIN_3, GPIO_PIN_SET);
  return result;
}

HAL_StatusTypeDef LIS_Read_data(uint8_t addr, uint8_t *data, uint16_t size)
{
  HAL_GPIO_WritePin(GPIOE, GPIO_PIN_3, GPIO_PIN_RESET);

  SPI_Transmit_Byte(addr | 0x80);
  HAL_StatusTypeDef result = HAL_SPI_Receive(&hspi1, data, size, HAL_MAX_DELAY);

  HAL_GPIO_WritePin(GPIOE, GPIO_PIN_3, GPIO_PIN_SET);
  return result;
}

HAL_StatusTypeDef LIS_Read_Byte(uint8_t addr, uint8_t *dest)
{
  HAL_GPIO_WritePin(GPIOE, GPIO_PIN_3, GPIO_PIN_RESET);

  SPI_Transmit_Byte(addr | 0x80);
  HAL_StatusTypeDef result = HAL_SPI_Receive(&hspi1, dest, 1, HAL_MAX_DELAY);

  HAL_GPIO_WritePin(GPIOE, GPIO_PIN_3, GPIO_PIN_SET);
  return result;
}

HAL_StatusTypeDef LIS_Read()
{
  if (LIS_Read_Byte(OUT_X_LOW, &x[1]))
  {
    LIS_HANDLE_ERROR;
  }
  if (LIS_Read_Byte(OUT_X_HIGH, &x[0]))
  {
    LIS_HANDLE_ERROR;
  }

  if (LIS_Read_Byte(OUT_Y_LOW, &y[1]))
  {
    LIS_HANDLE_ERROR;
  }
  if (LIS_Read_Byte(OUT_Y_HIGH, &y[0]))
  {
    LIS_HANDLE_ERROR;
  }

  if (LIS_Read_Byte(OUT_Z_LOW, &z[1]))
  {
    LIS_HANDLE_ERROR;
  }
  if (LIS_Read_Byte(OUT_Z_HIGH, &z[0]))
  {
    LIS_HANDLE_ERROR;
  }

  return HAL_OK;
}

uint8_t LIS_Debug_Log_USB()
{
  static uint8_t TxBuffer[48] = {0};
  sprintf((char *)TxBuffer,                                           //
          "Data: x = 0x%02X%02X, y = 0x%02X%02X, z = 0x%02X%02X\r\n", //
          x[0], x[1], y[0], y[1], z[0], z[1]);

  HAL_Delay(VTC_TIMEOUT);
  uint8_t result = CDC_Transmit_FS(TxBuffer, strlen((char *)TxBuffer) + 1);
  HAL_Delay(VTC_TIMEOUT);
  return result;
}

HAL_StatusTypeDef LIS_Check_Status_USB()
{
  static uint8_t errorMsg[] = "Failed to read LIS registers.\r\n";

  HAL_GPIO_WritePin(GPIOD, GPIO_PIN_12, GPIO_PIN_RESET);

  uint8_t result[3] = {0};
  if (LIS_Read_data(WHO_AM_I, &result[0], 1) != HAL_OK ||   //
      LIS_Read_data(STATUS_REG, &result[1], 1) != HAL_OK || //
      LIS_Read_data(CTRL_REG4, &result[2], 1) != HAL_OK)
  {
    HAL_Delay(VTC_TIMEOUT);
    CDC_Transmit_FS(errorMsg, sizeof(errorMsg));
    HAL_Delay(VTC_TIMEOUT);

    HAL_GPIO_WritePin(GPIOD, GPIO_PIN_13, GPIO_PIN_SET);
    return HAL_ERROR;
  }
  else
  {
    HAL_GPIO_WritePin(GPIOD, GPIO_PIN_12, GPIO_PIN_SET);
  }

  uint8_t resultBuf[48] = {0};
  sprintf((char *)resultBuf, "WHO_AM_I = 0x%02X | STAT = 0x%02X | CTRL4 = 0x%02X\r\n", result[0], result[1], result[2]);

  HAL_Delay(VTC_TIMEOUT);
  CDC_Transmit_FS(resultBuf, strlen((char *)resultBuf) + 1);
  HAL_Delay(VTC_TIMEOUT);

  return HAL_OK;
}
