#pragma once

/**************************************************************************************
* OpenDraft:    System Colour Class                                                   *
*-------------------------------------------------------------------------------------*
* Filename:     OdSyColour.h                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 9, 2023                                                          *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A colour class for the OpenDraft framework.                                       *
***************************************************************************************/



#include <nanovg.h>



class OdSyColour
{
private:
	int r, g, b, a;

public:
	// Default constructor
	OdSyColour() : r(0), g(0), b(0), a(255) {}

	// Custom constructor
	OdSyColour(int red, int green, int blue, int alpha = 255) : r(red), g(green), b(blue), a(alpha) {}

	// Getters and setters
	int getRed() const { return r; }
	void setRed(int red) { r = red; }

	int getGreen() const { return g; }
	void setGreen(int green) { g = green; }

	int getBlue() const { return b; }
	void setBlue(int blue) { b = blue; }

	int getAlpha() const { return a; }
	void setAlpha(int alpha) { a = alpha; }

	NVGcolor asNvgColour()
	{
		return nvgRGBA(getRed(), getGreen(), getBlue(), getAlpha());
	}
};
