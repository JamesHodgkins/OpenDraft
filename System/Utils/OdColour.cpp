#ifndef OD_SY_COLOUR_CPP
#define OD_SY_COLOUR_CPP

/**************************************************************************************
* OpenDraft:    System Colour Class                                                   *
*-------------------------------------------------------------------------------------*
* Filename:     OdColour.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 21, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A colour class for the OpenDraft framework.                                       *
***************************************************************************************/



#include "System/Utils/OdColour.h"

namespace OD
{
	namespace System
	{

		// Colour class
		const OdColour OdColour::BACKGROUND1 = OdColour(31, 39, 48, 255);
		const OdColour OdColour::BACKGROUND2 = OdColour(41, 51, 61, 255);
		const OdColour OdColour::PRIMARY = OdColour(113, 195, 61, 255);
		const OdColour OdColour::CLEAR = OdColour(0, 0, 0, 0);
		const OdColour OdColour::WHITE = OdColour(218, 224, 230, 255);

	} // namespace System
} // namespace OD

#endif // OD_SY_COLOUR_CPP