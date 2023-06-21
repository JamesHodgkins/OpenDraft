#ifndef OD_GR_UI_BUTTON_CPP
#define OD_GR_UI_BUTTON_CPP

/**************************************************************************************
* OpenDraft:    GUI Button Class                                                      *
*-------------------------------------------------------------------------------------*
* Filename:     OdButton.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 21, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI button class derived from OdComponent                                       *
***************************************************************************************/



#include "Graphics/UI/OdButton.h"

namespace OD
{
	namespace Graphics
	{
		OdButton::OdButton(int aX, int aY, int aWidth, int aHeight, std::string aText)
		{
			location.x = aX;
			location.y = aY;
			size.x = aWidth;
			size.y = aHeight;

			backColour = OdColour::BACKGROUND1;
			backColourHover = OdColour::BACKGROUND2;
			backColourActive = OdColour::BACKGROUND2;

			foreColour = OdColour::WHITE;
			foreColourHover = OdColour::WHITE;
			foreColourActive = OdColour::PRIMARY;

			stroke = OdColour(0, 0, 0, 0);

			backgroundImage = nullptr;

			text = aText;
		}

		void OdButton::setBackgroundImage(OdImage* aImage)
		{
			backgroundImage = aImage;
		}

		void OdButton::clearBackgroundImage()
		{
			backgroundImage = nullptr;
		}

		void OdButton::DrawButtonState(NVGcontext* aContext, float x, float y, const OdColour& backColour, const OdColour& foreColor)
		{
			OdDraw::Rect(aContext, x, y, size.x, size.y, backColour);
			OdDraw::Text(aContext, x, y, size.x, size.y, 14.0f, foreColor, text.c_str());
		}

		void const OdButton::onFrame(NVGcontext* aContext)
		{
			if (!enabled)
				return;

			if (aContext == nullptr)
				return;


			// Calculate draw location
			int x = getRelativeLocation().x;
			int y = getRelativeLocation().y;


			// Draw button state
			if (mouseDown)
				DrawButtonState(aContext, x, y, backColourActive, foreColourActive);

			else if (mouseOver)
				DrawButtonState(aContext, x, y - 1, backColourHover, foreColourHover);

			else
				DrawButtonState(aContext, x, y, backColour, foreColour);


			// Draw background image if set
			if (backgroundImage != nullptr)
				OdDraw::ResourceImage(aContext, x, y, size.x, size.y, backgroundImage);


			OdDraw::RectStroke(aContext, getRelativeLocation().x, getRelativeLocation().y, size.x, size.y, stroke);

		}

	} // namespace Graphics
} // namespace OpenDraft

#endif // OD_GR_UI_BUTTON_CPP