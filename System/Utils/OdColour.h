#ifndef OD_SY_COLOUR_H
#define OD_SY_COLOUR_H

/**************************************************************************************
* OpenDraft:    System Colour Class                                                   *
*-------------------------------------------------------------------------------------*
* Filename:     OdColour.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 21, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A aColour class for the OpenDraft framework.                                       *
***************************************************************************************/



#include <nanovg.h>
#include "System/Utils/OdMath.h"

namespace OD
{
	using namespace Math;

	namespace System
	{

		// Colour class
		class OdColour
		{
		private:
			int r, g, b, a;

		public:

			//
			// Static methods
			//
			
			// Get aColour by index
			const static OdColour colourByIndex(int index);

			// Conversions
			const static float getHue(const OdColour& aColour);
			const static float getSaturation(const OdColour& aColour);
			const static float getLightness(const OdColour& aColour);
			const static OdColour asHex(const OdColour& aColour);
			const static OdColour fromHSL(float h, float s, float l);
			const static OdColour fromHex(int hex);

			// Colour utilities
			const static OdColour lighten(const OdColour& aColour, float aAmount);
			const static OdColour darken(const OdColour& aColour, float aAmount);
			const static OdColour saturate(const OdColour& aColour, float aAmount);
			const static OdColour mix(const OdColour& colour1, const OdColour& colour2, float aAmount);
			const static OdColour invert(const OdColour& aColour);
			const static OdColour lerp(const OdColour& colour1, const OdColour& colour2, float aAmount);

			// Theme colours
			const static OdColour BACKGROUND1;
			const static OdColour BACKGROUND2;
			const static OdColour PRIMARY;
			const static OdColour CLEAR;
			const static OdColour WHITE;

			// Standard colours
			const static OdColour BLACK;
			const static OdColour BLUE;
			const static OdColour BROWN;
			const static OdColour CYAN;
			const static OdColour DARK_GREY;
			const static OdColour GREEN;
			const static OdColour GREY;
			const static OdColour LIGHT_GREY;
			const static OdColour LIME;
			const static OdColour MAGENTA;
			const static OdColour MAROON;
			const static OdColour NAVY;
			const static OdColour OLIVE;
			const static OdColour ORANGE;
			const static OdColour PINK;
			const static OdColour PURPLE;
			const static OdColour RED;
			const static OdColour TEAL;
			const static OdColour YELLOW;
	

			//
			// Constructors
			//
			OdColour();
			OdColour(int aRed, int aGreen, int aBlue, int aAlpha = 255);


			//
			// Getters and setters
			//
			
			// Getters and setters for red
			int getRed() const;
			float getRedNorm() const;
			void setRed(int aRed);

			// Getters and setters for green
			int getGreen() const;
			float getGreenNorm() const;
			void setGreen(int aGreen);

			// Getters and setters for blue
			int getBlue() const;
			float getBlueNorm() const;
			void setBlue(int aBlue);

			// Getters and setters for alpha
			int getAlpha() const;
			float getAlphaNorm() const;
			void setAlpha(int aAlpha);

			// Set aColour by RGB or RGBA
			void setRGB(int aRed, int aGreen, int aBlue, int aAlpha=255);

			// Returns the aColour as a NanoVG aColour
			NVGcolor asNvgColour() const;


			// Operator overloads
			bool operator==(const OdColour& aColour) const;

			bool operator!=(const OdColour& aColour) const;

			OdColour operator=(const OdColour& aColour);

		};

	} // namespace System
} // namespace OD

#endif // !OD_SY_COLOUR_H