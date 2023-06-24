#ifndef OD_SY_COLOUR_H
#define OD_SY_COLOUR_H

/**************************************************************************************
* OpenDraft:    System Colour Class                                                   *
*-------------------------------------------------------------------------------------*
* Filename:     OdColour.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 21, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A colour class for the OpenDraft framework.                                       *
***************************************************************************************/



#include <nanovg.h>

namespace OD
{
	namespace System
	{

		// Colour class
		class OdColour
		{
		private:
			int r, g, b, a;

		public:

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
	

			// Default constructor
			OdColour() : r(0), g(0), b(0), a(255) {}

			// Custom constructor
			OdColour(int aRed, int aGreen, int aBlue, int aAlpha = 255) : r(aRed), g(aGreen), b(aBlue), a(aAlpha) {}

			// Getters and setters
			int getRed() const { return r; }
			float getRedNorm() const { return static_cast<float>(r)/255.f; }
			void setRed(int aRed) { r = aRed; }

			int getGreen() const { return g; }
			float getGreenNorm() const { return static_cast<float>(g)/255.f; }
			void setGreen(int aGreen) { g = aGreen; }

			int getBlue() const { return b; }
			float getBlueNorm() const { return static_cast<float>(b)/255.f; }
			void setBlue(int aBlue) { b = aBlue; }

			int getAlpha() const { return a; }
			float getAlphaNorm() const { return static_cast<float>(a)/255.f; }
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

			NVGcolor asNvgColour() const
			{
				return nvgRGBA(getRed(), getGreen(), getBlue(), getAlpha());
			}
		};

	} // namespace System
} // namespace OD

#endif // OD_SY_COLOUR_H