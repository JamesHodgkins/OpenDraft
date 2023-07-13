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

			//
			// Static methods
			//
			
			// Get colour by index
			const static OdColour colourByIndex(int index);

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

			// Set colour by RGB or RGBA
			void setRGB(int aRed, int aGreen, int aBlue, int aAlpha=255);

			// Returns the colour as a NanoVG colour
			NVGcolor asNvgColour() const;


			// Operator overloads
			bool operator==(const OdColour& aColour) const;

			bool operator!=(const OdColour& aColour) const;

			OdColour operator=(const OdColour& aColour);

		};

	} // namespace System
} // namespace OD

#endif // OD_SY_COLOUR_H