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

			const static OdColour BACKGROUND1;
			const static OdColour BACKGROUND2;
			const static OdColour PRIMARY;
			const static OdColour CLEAR;
			const static OdColour WHITE;
	

			// Default constructor
			OdColour() : r(0), g(0), b(0), a(255) {}

			// Custom constructor
			OdColour(int aRed, int aGreen, int aBlue, int aAlpha = 255) : r(aRed), g(aGreen), b(aBlue), a(aAlpha) {}

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

			NVGcolor asNvgColour() const
			{
				return nvgRGBA(getRed(), getGreen(), getBlue(), getAlpha());
			}
		};

	} // namespace System
} // namespace OD

#endif // OD_SY_COLOUR_H