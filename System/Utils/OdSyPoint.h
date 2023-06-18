#pragma once

/**************************************************************************************
* OpenDraft:    System Point Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdSyPoint.h                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         June 9, 2023                                                          *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A point class for the OpenDraft framework, for locations, sizes etc               *
***************************************************************************************/



class OdSyPoint {
private:

public:
	double x, y;

	// Default constructor
	OdSyPoint() : x(0.0), y(0.0) {}

	// Custom constructor
	OdSyPoint(double xCoord, double yCoord) : x(xCoord), y(yCoord) {}
	OdSyPoint(int xCoord, int yCoord) : x(xCoord), y(yCoord) {}
};