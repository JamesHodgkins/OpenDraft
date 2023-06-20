#ifndef OD_SY_POINT_H
#define OD_SY_POINT_H

/**************************************************************************************
* OpenDraft:    System Point Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdPoint.h                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         June 9, 2023                                                          *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A point class for the OpenDraft framework, for locations, sizes etc               *
***************************************************************************************/


namespace OD
{
	namespace System
	{
		// Point class
		class OdPoint {
		private:

		public:
			double x, y;

			// Default constructor
			OdPoint() : x(0.0), y(0.0) {}

			// Custom constructor
			OdPoint(double aX, double aY) : x(aX), y(aY) {}
			OdPoint(int aX, int aY) : x(aX), y(aY) {}
		};

	} // namespace System
} // namespace OD

#endif // OD_SY_POINT_H