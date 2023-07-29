#ifndef OD_SY_MATH_H
#define OD_SY_MATH_H

/**************************************************************************************
* OpenDraft:    System Math Class                                                     *
*-------------------------------------------------------------------------------------*
* Filename:     OdMath.h                                                              *
* Contributors: James Hodgkins                                                        *
* Date:         July 29, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A math class for the OpenDraft framework.                                         *
***************************************************************************************/


#include <cmath>

// Define constants
#define OD_PI 3.14159265358979323846
#define OD_E 2.71828182845904523536
#define OD_RAD2DEG_FACTOR 0.01745329251994329577
#define OD_DEG2RAD_FACTOR 57.2957795130823208768

namespace OD
{
	namespace Math
	{

		class OdMath
		{
		// Static methods
		public:
			// Conversions
			static float deg2rad(float deg)
			{
				return deg; // *OD_DEG2RAD_FACTOR;
			}

			static float rad2deg(float rad)
			{
				return rad; // *OD_RAD2DEG_FACTOR;
			}


			// Trigonometry
			static float sin(float angle)
			{
				return sinf(angle);
			}

			static float cos(float angle)
			{
				return cosf(angle);
			}

			static float tan(float angle)
			{
				return tanf(angle);
			}

			static float asin(float angle)
			{
				return asinf(angle);
			}

			static float acos(float angle)
			{
				return acosf(angle);
			}
			static float atan(float angle)
			{
				return atanf(angle);
			}
			static float atan2(float y, float x)
			{
				return atan2f(y, x);
			}

			// Rounding
			static float ceil(float value)
			{
				return ceilf(value);
			}

			static float floor(float value)
			{
				return floorf(value);
			}

			static float round(float value)
			{
				return roundf(value);
			}

			static float round(float value, int precision)
			{
				return roundf(value * pow(10, precision)) / pow(10, precision);
			}

			static float round(float value, int precision, bool truncate)
			{
				if (truncate)
				{
					return floorf(value * pow(10, precision)) / pow(10, precision);
				}
				else
				{
					return roundf(value * pow(10, precision)) / pow(10, precision);
				}
			}




			// Exponents
			static float pow(float base, float exponent)
			{
				return powf(base, exponent);
			}

			static float sqrt(float value)
			{
				return sqrtf(value);
			}

			static float cbrt(float value)
			{
				return cbrtf(value);
			}

			static float root(float value, float root)
			{
				return powf(value, 1 / root);
			}

			// Logarithms
			static float log(float value)
			{
				return logf(value);
			}

			static float log(float value, float base)
			{
				return logf(value) / logf(base);
			}

			static float log2(float value)
			{
				return logf(value) / logf(2);
			}

			static float log10(float value)
			{
				return log10f(value);
			}

			// Absolute value
			static float abs(float value)
			{
				return fabsf(value);
			}

			// Min/max
			//static float min(float value1, float value2)
			//{
			//	return fminf(value1, value2);
			//}

			//static float max(float value1, float value2)
			//{
			//	return fmaxf(value1, value2);
			//}

			//// Clamp
			//static float clamp(float value, float min, float max)
			//{
			//	return fminf(fmaxf(value, min), max);
			//}

			//// Linear interpolation
			//static float lerp(float value1, float value2, float amount)
			//{
			//	return value1 + (value2 - value1) * amount;
			//}

			// Random
			/*static float random()
			{
				return (float)rand() / RAND_MAX;
			}

			static float random(float min, float max)
			{
				return min + (max - min) * random();
			}

			static float random(float min, float max, int seed)
			{
				srand(seed);
				return min + (max - min) * random();
			}*/

			// Modulus
			//static float modulus(float value1, float value2)
			//{
			//	return fmodf(value1, value2);
			//}

			//// Sign
			//static int sign(float value)
			//{
			//	return value < 0 ? -1 : 1;
			//}

			//// Is even
			//static bool isEven(float value)
			//{
			//	return fmodf(value, 2) == 0;
			//}


		};
	}
}


#endif // !OD_SY_MATH_H