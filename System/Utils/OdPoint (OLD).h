#ifndef OD_SY_POINT_H
#define OD_SY_POINT_H

/**************************************************************************************
* OpenDraft:    System Point Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdPoint.h                                                             *
* Contributors: James Hodgkins                                                        *
* Date:         June 9, 2023                                                          *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A point class for the OpenDraft framework, for locations, sizes etc               *
***************************************************************************************/


namespace OD::System
{
	// Point class
	template<typename T>
	class OdPoint {
	private:

	public:
		T x, y;

		// Default constructor
		OdPoint() : x(0), y(0) {}

		// Custom constructor
		OdPoint(T aX, T aY) : x(aX), y(aY) {}
	};

} // namespace OD::System

#endif // OD_SY_POINT_H