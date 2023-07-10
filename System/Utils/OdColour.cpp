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


		// Default constructor
		OdColour::OdColour() : r(0), g(0), b(0), a(255) {}

		// Custom constructor
		OdColour::OdColour(int aRed, int aGreen, int aBlue, int aAlpha) : r(aRed), g(aGreen), b(aBlue), a(aAlpha) {}

		// Getters and setters
		int OdColour::getRed() const { return r; }
		float OdColour::getRedNorm() const { return static_cast<float>(r) / 255.f; }
		void OdColour::setRed(int aRed) { r = aRed; }
		int OdColour::getGreen() const { return g; }
		float OdColour::getGreenNorm() const { return static_cast<float>(g) / 255.f; }
		void OdColour::setGreen(int aGreen) { g = aGreen; }
		int OdColour::getBlue() const { return b; }
		float OdColour::getBlueNorm() const { return static_cast<float>(b) / 255.f; }
		void OdColour::setBlue(int aBlue) { b = aBlue; }
		int OdColour::getAlpha() const { return a; }
		float OdColour::getAlphaNorm() const { return static_cast<float>(a) / 255.f; }
		void OdColour::setAlpha(int aAlpha) { a = aAlpha; }
		void OdColour::setRGB(int aRed, int aGreen, int aBlue, int aAlpha)
		{
			r = aRed;
			g = aGreen;
			b = aBlue;
			a = aAlpha;
		}

		// Returns the colour as NVGcolor
		NVGcolor OdColour::asNvgColour() const
		{
			return nvgRGBA(getRed(), getGreen(), getBlue(), getAlpha());
		}


		// Operator overloads
		bool OdColour::operator==(const OdColour& aColour) const
		{
			return (r == aColour.r && g == aColour.g && b == aColour.b && a == aColour.a);
		}

		bool OdColour::operator!=(const OdColour& aColour) const
		{
			return !(*this == aColour);
		}

		OdColour OdColour::operator=(const OdColour& aColour)
		{
			r = aColour.r;
			g = aColour.g;
			b = aColour.b;
			a = aColour.a;
			return *this;
		}


	} // namespace System
} // namespace OD

#endif // OD_SY_COLOUR_CPP