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
			float deg2rad(float deg);
			float rad2deg(float rad);

			// Trigonometry
			float sin(float angle);
			float cos(float angle);
			float tan(float angle);
			float asin(float angle);
			float acos(float angle);
			float atan(float angle);
			float atan2(float y, float x);

			// Rounding
			float ceil(float value);
			float floor(float value);
			float round(float value);
			float round(float value, int precision);
			float round(float value, int precision, bool truncate);

			// Exponents
			float pow(float base, float exponent);
			float sqrt(float value);
			float cbrt(float value);
			float root(float value, float root);

			// Logarithms
			float log(float value);
			float log(float value, float base);
			float log2(float value);
			float log10(float value);

			// Absolute value
			float abs(float value);

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