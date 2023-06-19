#pragma once

/**************************************************************************************
* OpenDraft:    System Colour Class                                                   *
*-------------------------------------------------------------------------------------*
* Filename:     OdSyColour.h                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 19, 2023                                                         *
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

	const static OdSyColour BACKGROUND1;
	const static OdSyColour BACKGROUND2;
	const static OdSyColour PRIMARY;
	const static OdSyColour TRANSP;
	const static OdSyColour WHITE;
	

	// Default constructor
	OdSyColour() : r(0), g(0), b(0), a(255) {}

	// Custom constructor
	OdSyColour(int red, int green, int blue, int alpha = 255) : r(red), g(green), b(blue), a(alpha) {}

	// Getters and setters
	int getRed() const { return r; }
	float getRedNorm() const { return r/255.; }
	void setRed(int red) { r = red; }

	int getGreen() const { return g; }
	float getGreenNorm() const { return g / 255.; }
	void setGreen(int green) { g = green; }

	int getBlue() const { return b; }
	float getBlueNorm() const { return b / 255.; }
	void setBlue(int blue) { b = blue; }

	int getAlpha() const { return a; }
	float getAlphaNorm() const { return a / 255.; }
	void setAlpha(int alpha) { a = alpha; }

	void setRGB(int red, int green, int blue)
	{
		r = red;
		g = green;
		b = blue;
		a = 255;
	}

	void setColourRGBA(int red, int green, int blue, int alpha)
	{
		r = red;
		g = green;
		b = blue;
		a = alpha;
	}

	NVGcolor asNvgColour()
	{
		return nvgRGBA(getRed(), getGreen(), getBlue(), getAlpha());
	}
};

const OdSyColour OdSyColour::BACKGROUND1 = OdSyColour(31, 39, 48, 255);
const OdSyColour OdSyColour::BACKGROUND2 = OdSyColour(41, 51, 61, 255);
const OdSyColour OdSyColour::PRIMARY = OdSyColour(113, 195, 61, 255);
const OdSyColour OdSyColour::TRANSP = OdSyColour(0, 0, 0, 0);
const OdSyColour OdSyColour::WHITE = OdSyColour(218, 224, 230, 255);
