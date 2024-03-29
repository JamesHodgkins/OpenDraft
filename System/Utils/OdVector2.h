#ifndef OD_SY_VECTOR2_H
#define OD_SY_VECTOR2_H

/**************************************************************************************
* OpenDraft:    System Vector2 Class                                                  *
*-------------------------------------------------------------------------------------*
* Filename:     OdVector2.h                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    �2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A 2D vector class for the OpenDraft framework for geometry elements               *
***************************************************************************************/


#include <string>


namespace OD::System
{
	// Vector2 class
	class OdVector2 {
	private:

	public:
		double x, y;

		// Constructors
		OdVector2();
		OdVector2(float aX, float aY);
		OdVector2(double aX, double aY);
		OdVector2(int aX, int aY);

		//
		// General Methods
		//

		// ToString
		std::string ToString();


		//
		// Vector Math
		//

		// Magnitude
		double magnitude();

		// Normalized
		OdVector2 normalized();

		// Dot product
		double dot(OdVector2 aVector);

		// Cross product
		double cross(OdVector2 aVector);

		// Angle between vectors
		double angle(OdVector2 aVector);

		// Lerp
		OdVector2 lerp(OdVector2 aVector, double aT);

		// Rotate
		OdVector2 rotate(double aAngle);

		// RotateAround
		OdVector2 rotateAround(OdVector2 aPoint, double aAngle);

		// Scale
		OdVector2 scale(OdVector2 aScale);

		// Distance to
		double distanceTo(const OdVector2& other) const;

		//
		// Operators
		//

		// operator+
		OdVector2 operator+(OdVector2 aVector);

		// operator-
		OdVector2 operator-(OdVector2 aVector);

		// operator*
		OdVector2 operator*(double aScalar);

		// operator/
		OdVector2 operator/(double aScalar);

		// operator+=
		void operator+=(OdVector2 aVector);

		// operator-=
		void operator-=(OdVector2 aVector);

		// operator*=
		void operator*=(double aScalar);

		// operator/=
		void operator/=(double aScalar);

		// operator==
		bool operator==(OdVector2 aVector);

		// operator!=
		bool operator!=(OdVector2 aVector);
		
	};

} // namespace OD::System

#endif // !OD_SY_VECTOR2_H
