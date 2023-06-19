#ifndef OD_SY_COLOUR_H
#define OD_SY_COLOUR_H

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
	const static OdSyColour CLEAR;
	const static OdSyColour WHITE;
	

	// Default constructor
	OdSyColour() : r(0), g(0), b(0), a(255) {}

	// Custom constructor
	OdSyColour(int aRed, int aGreen, int aBlue, int aAlpha = 255) : r(aRed), g(aGreen), b(aBlue), a(aAlpha) {}

	// Getters and setters
	int getRed() const { return r; }
	float getRedNorm() const { return r/255.; }
	void setRed(int aRed) { r = aRed; }

	int getGreen() const { return g; }
	float getGreenNorm() const { return g / 255.; }
	void setGreen(int aGreen) { g = aGreen; }

	int getBlue() const { return b; }
	float getBlueNorm() const { return b / 255.; }
	void setBlue(int aBlue) { b = aBlue; }

	int getAlpha() const { return a; }
	float getAlphaNorm() const { return a / 255.; }
	void setAlpha(int aAlpha) { a = aAlpha; }

	void setRGB(int aRed, int aGreen, int aBlue)
	{
		r = aRed;
		g = aGreen;
		b = aBlue;
		a = 255;
	}

	void setColourRGBA(int aRed, int aGreen, int aBlue, int aAlpha)
	{
		r = aRed;
		g = aGreen;
		b = aBlue;
		a = aAlpha;
	}

	NVGcolor asNvgColour()
	{
		return nvgRGBA(getRed(), getGreen(), getBlue(), getAlpha());
	}
};

const OdSyColour OdSyColour::BACKGROUND1 = OdSyColour(31, 39, 48, 255);
const OdSyColour OdSyColour::BACKGROUND2 = OdSyColour(41, 51, 61, 255);
const OdSyColour OdSyColour::PRIMARY = OdSyColour(113, 195, 61, 255);
const OdSyColour OdSyColour::CLEAR = OdSyColour(0, 0, 0, 0);
const OdSyColour OdSyColour::WHITE = OdSyColour(218, 224, 230, 255);

#endif // OD_SY_COLOUR_H