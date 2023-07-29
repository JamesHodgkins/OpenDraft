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
constexpr double OD_PI = 3.14159265358979323846;
constexpr double OD_E = 2.71828182845904523536;
constexpr double OD_RAD2DEG_FACTOR = 0.01745329251994329577;
constexpr double OD_DEG2RAD_FACTOR = 57.2957795130823208768;

namespace OD
{
	namespace Math
	{

		class OdMath
		{

		// Static methods
		public:

			// Conversions
			static inline float deg2rad(float aDeg) { return aDeg * OD_RAD2DEG_FACTOR; }
			static inline float rad2deg(float aRad) { return aRad * OD_DEG2RAD_FACTOR; }

			// Trigonometry
			static float sin(float aAngle);
			static float cos(float aAngle);
			static float tan(float aAngle);
			static float asin(float aAngle);
			static float acos(float aAngle);
			static float atan(float aAngle);
			static float atan2(float aY, float aX);

			// Rounding
			static float ceil(float aValue);
			static float floor(float aValue);
			static float round(float aValue);
			static float round(float aValue, int aPrecision);
			static float round(float aValue, int aPrecision, bool aTruncate);

			// Min/max and clamp
			static float min2(float aValue1, float aValue2);
			static float max2(float aValue1, float aValue2);
			static float min3(float aValue1, float aValue2, float aValue3);
			static float max3(float aValue1, float aValue2, float aValue3);
			static float clamp(float aValue, float aMin, float aMax);

			// Exponents
			static float pow(float aBase, float aExponent);
			static float sqrt(float aValue);
			static float cbrt(float aValue);

			// Logarithms
			static float log(float aValue);
			static float log(float aValue, float aBase);
			static float log2(float aValue);
			static float log10(float aValue);

			// Absolute value
			static float abs(float aValue);

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