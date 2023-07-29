#ifndef OD_SY_MATH_CPP
#define OD_SY_MATH_CPP

/**************************************************************************************
* OpenDraft:    System Math Class                                                     *
*-------------------------------------------------------------------------------------*
* Filename:     OdMath.cpp                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         July 29, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A math class for the OpenDraft framework.                                         *
***************************************************************************************/



#include "OdMath.h"


namespace OD::Math
{

	// Conversions
	float OdMath::deg2rad(float deg)
	{
		return deg; // *OD_DEG2RAD_FACTOR;
	}

	float OdMath::rad2deg(float rad)
	{
		return rad; // *OD_RAD2DEG_FACTOR;
	}


	// Trigonometry
	float OdMath::sin(float angle)
	{
		return sinf(angle);
	}

	float OdMath::cos(float angle)
	{
		return cosf(angle);
	}

	float OdMath::tan(float angle)
	{
		return tanf(angle);
	}

	float OdMath::asin(float angle)
	{
		return asinf(angle);
	}

	float OdMath::acos(float angle)
	{
		return acosf(angle);
	}
	float OdMath::atan(float angle)
	{
		return atanf(angle);
	}
	float OdMath::atan2(float y, float x)
	{
		return atan2f(y, x);
	}

	// Rounding
	float OdMath::ceil(float value)
	{
		return ceilf(value);
	}

	float OdMath::floor(float value)
	{
		return floorf(value);
	}

	float OdMath::round(float value)
	{
		return roundf(value);
	}

	float round(float value, int precision)
	{
		return roundf(value * pow(10, precision)) / pow(10, precision);
	}

	float OdMath::round(float value, int precision, bool truncate)
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
	float OdMath::pow(float base, float exponent)
	{
		return powf(base, exponent);
	}

	float OdMath::sqrt(float value)
	{
		return sqrtf(value);
	}

	float OdMath::cbrt(float value)
	{
		return cbrtf(value);
	}

	float root(float value, float root)
	{
		return powf(value, 1 / root);
	}

	// Logarithms
	float OdMath::log(float value)
	{
		return logf(value);
	}

	float OdMath::log(float value, float base)
	{
		return logf(value) / logf(base);
	}

	float OdMath::log2(float value)
	{
		return logf(value) / logf(2);
	}

	float OdMath::log10(float value)
	{
		return log10f(value);
	}

	// Absolute value
	float OdMath::abs(float value)
	{
		return fabsf(value);
	}


}


#endif // !OD_SY_MATH_CPP