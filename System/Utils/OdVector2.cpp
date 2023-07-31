#ifndef OD_SY_VECTOR2_CPP
#define OD_SY_VECTOR2_CPP

/**************************************************************************************
* OpenDraft:    System Vector2 Class                                                  *
*-------------------------------------------------------------------------------------*
* Filename:     OdVector2.cpp                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A 2D vector class for the OpenDraft framework for geometry elements               *
***************************************************************************************/


#include "System/Utils/OdVector2.h"


namespace OD::System
{
	// Constructors
	OdVector2::OdVector2() : x(0), y(0) {}
	OdVector2::OdVector2(float aX, float aY) : x(aX), y(aY) {}
	OdVector2::OdVector2(double aX, double aY) : x(aX), y(aY) {}
	OdVector2::OdVector2(int aX, int aY) : x(aX), y(aY) {}


	//
	// General Methods
	//

	// ToString
	std::string OdVector2::ToString()
	{
		return "(" + std::to_string(x) + ", " + std::to_string(y) + ")";
	}



	//
	// Vector Math
	//

	// Magnitude
	double OdVector2::magnitude()
	{
		return sqrt(x * x + y * y);
	}

	// Normalized
	OdVector2 OdVector2::normalized()
	{
		return OdVector2(x / magnitude(), y / magnitude());
	}

	// Dot product
	double OdVector2::dot(OdVector2 aVector)
	{
		return x * aVector.x + y * aVector.y;
	}

	// Cross product
	double OdVector2::cross(OdVector2 aVector)
	{
		return x * aVector.y - y * aVector.x;
	}

	// Angle between vectors
	double OdVector2::angle(OdVector2 aVector)
	{
		return acos(dot(aVector) / (magnitude() * aVector.magnitude()));
	}

	// Lerp
	OdVector2 OdVector2::lerp(OdVector2 aVector, double aT)
	{
		return OdVector2(x + (aVector.x - x) * aT, y + (aVector.y - y) * aT);
	}

	// Rotate
	OdVector2 OdVector2::rotate(double aAngle)
	{
		return OdVector2(x * cos(aAngle) - y * sin(aAngle), x * sin(aAngle) + y * cos(aAngle));
	}

	// RotateAround
	OdVector2 OdVector2::rotateAround(OdVector2 aPoint, double aAngle)
	{
		OdVector2 offset = *this - aPoint;
		double cosAngle = cos(aAngle);
		double sinAngle = sin(aAngle);
		double newX = offset.x * cosAngle - offset.y * sinAngle;
		double newY = offset.x * sinAngle + offset.y * cosAngle;
		OdVector2 rotatedVector = aPoint + OdVector2(newX, newY);
		return rotatedVector;
	}

	// Scale
	OdVector2 OdVector2::scale(OdVector2 aScale)
	{
		return OdVector2(x * aScale.x, y * aScale.y);
	}



	//
	// Operators
	//

	// operator+
	OdVector2 OdVector2::operator+(OdVector2 aVector)
	{
		return OdVector2(x + aVector.x, y + aVector.y);
	}

	// operator-
	OdVector2 OdVector2::operator-(OdVector2 aVector)
	{
		return OdVector2(x - aVector.x, y - aVector.y);
	}

	// operator*
	OdVector2 OdVector2::operator*(double aScalar)
	{
		return OdVector2(x * aScalar, y * aScalar);
	}

	// operator/
	OdVector2 OdVector2::operator/(double aScalar)
	{
		return OdVector2(x / aScalar, y / aScalar);
	}

	// operator+=
	void OdVector2::operator+=(OdVector2 aVector)
	{
		x += aVector.x;
		y += aVector.y;
	}

	// operator-=
	void OdVector2::operator-=(OdVector2 aVector)
	{
		x -= aVector.x;
		y -= aVector.y;
	}

	// operator*=
	void OdVector2::operator*=(double aScalar)
	{
		x *= aScalar;
		y *= aScalar;
	}

	// operator/=
	void OdVector2::operator/=(double aScalar)
	{
		x /= aScalar;
		y /= aScalar;
	}

	// operator==
	bool OdVector2::operator==(OdVector2 aVector)
	{
		return (x == aVector.x && y == aVector.y);
	}

	// operator!=
	bool OdVector2::operator!=(OdVector2 aVector)
	{
		return !(x == aVector.x && y == aVector.y);
	}


} // namespace OD::System

#endif // !OD_SY_VECTOR2_cpp