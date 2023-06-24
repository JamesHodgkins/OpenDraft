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

		// Theme colours
		const OdColour OdColour::BACKGROUND1 = OdColour(31, 39, 48, 255);
		const OdColour OdColour::BACKGROUND2 = OdColour(41, 51, 61, 255);
		const OdColour OdColour::PRIMARY = OdColour(113, 195, 61, 255);
		const OdColour OdColour::CLEAR = OdColour(0, 0, 0, 0);
		const OdColour OdColour::WHITE = OdColour(218, 224, 230, 255);

		// Standard colours
		const OdColour OdColour::BLACK = OdColour(0, 0, 0, 255);
		const OdColour OdColour::BLUE = OdColour(0, 0, 255, 255);
		const OdColour OdColour::BROWN = OdColour(165, 42, 42, 255);
		const OdColour OdColour::CYAN = OdColour(0, 255, 255, 255);
		const OdColour OdColour::DARK_GREY = OdColour(169, 169, 169, 255);
		const OdColour OdColour::GREEN = OdColour(90, 190, 70, 255);
		const OdColour OdColour::GREY = OdColour(128, 128, 128, 255);
		const OdColour OdColour::LIGHT_GREY = OdColour(211, 211, 211, 255);
		const OdColour OdColour::LIME = OdColour(0, 255, 0, 255);
		const OdColour OdColour::MAGENTA = OdColour(255, 0, 255, 255);
		const OdColour OdColour::MAROON = OdColour(128, 0, 0, 255);
		const OdColour OdColour::NAVY = OdColour(0, 0, 128, 255);
		const OdColour OdColour::OLIVE = OdColour(128, 128, 0, 255);
		const OdColour OdColour::ORANGE = OdColour(255, 165, 0, 255);
		const OdColour OdColour::PINK = OdColour(255, 192, 203, 255);
		const OdColour OdColour::PURPLE = OdColour(128, 0, 128, 255);
		const OdColour OdColour::RED = OdColour(255, 0, 0, 255);
		const OdColour OdColour::TEAL = OdColour(0, 128, 128, 255);
		const OdColour OdColour::YELLOW = OdColour(255, 255, 0, 255);


	} // namespace System
} // namespace OD

#endif // OD_SY_COLOUR_CPP