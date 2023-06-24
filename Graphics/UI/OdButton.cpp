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

namespace OD::Graphics
{
	OdButton::OdButton(const int aX, const int aY, const int aWidth, const int aHeight, const std::string aText)
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
		fontSize = 14;
	}

	void OdButton::setBackgroundImage(OdImage* aImage)
	{
		backgroundImage = aImage;
	}

	void OdButton::clearBackgroundImage()
	{
		backgroundImage = nullptr;
	}

	void OdButton::DrawButtonState(NVGcontext* aContext, const float x, const float y, const OdColour& backColour, const OdColour& foreColor)
	{
		// Static cast parameters to int
		int w = static_cast<int>(size.x);
		int h = static_cast<int>(size.y);

		OdDraw::Rect(aContext, x, y, w, h, backColour);
		OdDraw::Text(aContext, x, y, w, h, fontSize, foreColor, text.c_str());
	}

	void OdButton::onFrame(NVGcontext* aContext)
	{
		if (!enabled)
			return;

		if (aContext == nullptr)
			return;


		// Calculate draw location
		int x = static_cast<int>(getRelativeLocation().x);
		int y = static_cast<int>(getRelativeLocation().y);


		// Draw button state
		if (mouseDown)
			DrawButtonState(aContext, x, y, backColourActive, foreColourActive);

		else if (mouseOver)
			DrawButtonState(aContext, x, y, backColourHover, foreColourHover);

		else
			DrawButtonState(aContext, x, y, backColour, foreColour);


		// Draw background image if set
		if (backgroundImage != nullptr)
			OdDraw::ResourceImage(aContext, x, y, (int)size.x, (int)size.y, backgroundImage);


		OdDraw::RectStroke(aContext, x, y, (int)size.x, (int)size.y, stroke);

	}

} // namespace OD::Graphics

#endif // OD_GR_UI_BUTTON_CPP