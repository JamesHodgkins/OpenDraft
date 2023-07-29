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
	// Trigonometry
	float OdMath::sin(float aAngle)
	{
		return sinf(aAngle);
	}

	float OdMath::cos(float aAngle)
	{
		return cosf(aAngle);
	}

	float OdMath::tan(float aAngle)
	{
		return tanf(aAngle);
	}

	float OdMath::asin(float aAngle)
	{
		return asinf(aAngle);
	}

	float OdMath::acos(float aAngle)
	{
		return acosf(aAngle);
	}
	float OdMath::atan(float aAngle)
	{
		return atanf(aAngle);
	}
	float OdMath::atan2(float aY, float aX)
	{
		return atan2f(aY, aX);
	}

	// Rounding
	float OdMath::ceil(float aValue)
	{
		return ceilf(aValue);
	}

	float OdMath::floor(float aValue)
	{
		return floorf(aValue);
	}

	float OdMath::round(float aValue)
	{
		return roundf(aValue);
	}

	float OdMath::round(float aValue, int aPrecision)
	{
		return roundf(aValue * pow(10, aPrecision)) / pow(10, aPrecision);
	}

	float OdMath::round(float aValue, int aPrecision, bool aTruncate)
	{
		if (aTruncate)
		{
			return floorf(aValue * pow(10, aPrecision)) / pow(10, aPrecision);
		}
		else
		{
			return roundf(aValue * pow(10, aPrecision)) / pow(10, aPrecision);
		}
	}


	// Min/max and clamp
	float OdMath::min2(float aValue1, float aValue2)
	{
		// Calculate the minimum value
		return (aValue1 < aValue2)? aValue1 : aValue2;
	}
	
	float OdMath::max2(float aValue1, float aValue2)
	{
		// Calculate the maximum value
		return (aValue1 > aValue2)? aValue1 : aValue2;
	}
	
	float OdMath::min3(float aValue1, float aValue2, float aValue3)
	{
		return (aValue1 < aValue2)? ((aValue1 < aValue3)? aValue1 : aValue3) : ((aValue2 < aValue3)? aValue2 : aValue3);
	}
	
	float OdMath::max3(float aValue1, float aValue2, float aValue3)
	{
		return (aValue1 > aValue2)? ((aValue1 > aValue3)? aValue1 : aValue3) : ((aValue2 > aValue3)? aValue2 : aValue3);
	}

	float OdMath::clamp(float aValue, float aMin, float aMax)
	{
		if (aValue < aMin)
			return aMin;
		else if (aValue > aMax)
			return aMax;
		else
			return aValue;
	}




	// Exponents
	float OdMath::pow(float aBase, float aExponent)
	{
		return powf(aBase, aExponent);
	}

	float OdMath::sqrt(float aValue)
	{
		return sqrtf(aValue);
	}

	float OdMath::cbrt(float aValue)
	{
		return cbrtf(aValue);
	}

	float root(float aValue, float aRoot)
	{
		return powf(aValue, 1 / aRoot);
	}

	// Logarithms
	float OdMath::log(float aValue)
	{
		return logf(aValue);
	}

	float OdMath::log(float aValue, float aBase)
	{
		return logf(aValue) / logf(aBase);
	}

	float OdMath::log2(float aValue)
	{
		return logf(aValue) / logf(2);
	}

	float OdMath::log10(float aValue)
	{
		return log10f(aValue);
	}

	// Absolute value
	float OdMath::abs(float aValue)
	{
		return fabsf(aValue);
	}


}


#endif // !OD_SY_MATH_CPP