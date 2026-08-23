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

#define CR4_RATE      0x60 // 100 Hz
#define CR4_XYZ_ON    0x07
#define WAIT_FOR_READ 0x08

// Timeout before and after sending USB VTC msg.
//
// Explanation: We use a small delay between consecutive transmits
// on the USB CDC peripheral, because they are buffered and sent
// asynchronously, and without the delay the immediate second write
// encounters a busy device and fails.

#define VTC_TIMEOUT 2

// Global sensor state variables.

uint8_t x[2] = {0};
uint8_t y[2] = {0};
uint8_t z[2] = {0};

// Scale raw axis readings to g unit.

static const uint16_t g_SCALE_X = 17700;
static const uint16_t g_SCALE_Y = 16500;
static const uint16_t g_SCALE_Z = 17300;

// SPI handle from main application.

static SPI_HandleTypeDef hspi1;

// Reading data conversion functions.

int16_t convert(uint8_t *reading)
{
  uint16_t combined = (reading[0] << 8) | reading[1];
  return (int16_t)combined;
}

// For use in live debugging and calibration.
float convert_float(uint8_t *reading, uint16_t normalizer)
{
  uint16_t combined = (reading[0] << 8) | reading[1];
  return (int16_t)combined / (float)normalizer;
}

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
  if (SPI_Transmit_Byte(CTRL_REG4) || SPI_Transmit_Byte(CR4_RATE | CR4_XYZ_ON | WAIT_FOR_READ))
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

#define FILL_BUFFER(var, ind)                                                                                          \
  if (var[ind] == 0x00 || var[ind] == 0xFF)                                                                            \
  {                                                                                                                    \
    TxBuffer[current_idx]     = 0xFF;                                                                                  \
    TxBuffer[current_idx + 1] = var[ind];                                                                              \
    current_idx += 2;                                                                                                  \
  }                                                                                                                    \
  else                                                                                                                 \
  {                                                                                                                    \
    TxBuffer[current_idx] = var[ind];                                                                                  \
    current_idx += 1;                                                                                                  \
  }

uint8_t LIS_Send_Readings_USB()
{
  static uint8_t TxBuffer[64] = {0};

  /*
   * Send data packet using simple escape-based byte stuffing, in
   * case receiver starts receiving in the middle of a message.
   *
   * Special bytes are:
   *  - 0xx0 indicates beginning of message; 0x00 0x00 indicates end.
   *  - 0xFF is the escape byte that proceeds a 0x00 or 0xFF in message.
   */

  uint16_t current_idx = 1;
  FILL_BUFFER(x, 0)
  FILL_BUFFER(x, 1)
  FILL_BUFFER(y, 0)
  FILL_BUFFER(y, 1)
  FILL_BUFFER(z, 0)
  FILL_BUFFER(z, 1)
  TxBuffer[current_idx]     = 0x00;
  TxBuffer[current_idx + 1] = 0x00;
  current_idx += 2;

  uint8_t result = CDC_Transmit_FS(TxBuffer, current_idx);
  return result;
}

uint8_t LIS_Send_Debug_USB()
{
  static uint8_t TxBuffer[64] = {0};

#ifdef SEND_RAW_HEX
  sprintf((char *)TxBuffer,
          "Data: x = 0x%02X%02X, y = 0x%02X%02X, z = 0x%02X%02X\r\n", //
          x[0], x[1], y[0], y[1], z[0], z[1]);
#endif
#ifdef SEND_RAW_SIGNED
  sprintf((char *)TxBuffer, "Data: x = %06d, y = %06d, z = %06d\r\n", convert(x), convert(y), convert(z));
#endif

  /*
   * Done this way for live debugging. In practice we'll send the
   * raw values and let the client do the heavy math on its end.
   */

  float x_g_scaled = convert_float(x, g_SCALE_X);
  float y_g_scaled = convert_float(y, g_SCALE_Y);
  float z_g_scaled = convert_float(z, g_SCALE_Z);

  // Requires linker flag `-u _printf_float`.
  sprintf((char *)TxBuffer, "Data: x = %1.4f, y = %1.4f, z = %1.4f\r\n", x_g_scaled, y_g_scaled, z_g_scaled);

  uint8_t result = CDC_Transmit_FS(TxBuffer, strlen((char *)TxBuffer) + 1);
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
    CDC_Transmit_FS(errorMsg, sizeof(errorMsg));

    HAL_GPIO_WritePin(GPIOD, GPIO_PIN_13, GPIO_PIN_SET);
    return HAL_ERROR;
  }
  else
  {
    HAL_GPIO_WritePin(GPIOD, GPIO_PIN_12, GPIO_PIN_SET);
  }

  uint8_t resultBuf[48] = {0};
  sprintf((char *)resultBuf, "WHO_AM_I = 0x%02X | STAT = 0x%02X | CTRL4 = 0x%02X\r\n", result[0], result[1], result[2]);

  CDC_Transmit_FS(resultBuf, strlen((char *)resultBuf) + 1);

  return HAL_OK;
}
