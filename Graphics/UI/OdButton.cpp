#ifndef OD_GR_UI_BUTTON_CPP
#define OD_GR_UI_BUTTON_CPP

/**************************************************************************************
* OpenDraft:    GUI Button Class                                                      *
*-------------------------------------------------------------------------------------*
* Filename:     OdButton.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 21, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
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

		padding = 10;

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

	void OdButton::DrawButtonState(NVGcontext* aContext, const float x, const float y, const OdColour& backColour, const OdColour& foreColour)
	{
		// Static cast parameters to int
		int w = static_cast<int>(size.x);
		int h = static_cast<int>(size.y);

		OdDraw::TextStyle buttonTextStyle =
		{
			fontSize,
			"sans",
			foreColour,
			OdAlign(OdAlign::CENTRE | OdAlign::MIDDLE)
		};

		OdDraw::Rect(aContext, x, y, w, h, backColour);
		OdDraw::Text(aContext, x, y, w, h, &buttonTextStyle, text.c_str());
	}

	void OdButton::onFrame(NVGcontext* aContext)
	{
		// Calculate draw location
		float x = getLocation().x;
		float y = getLocation().y;

		// Draw button state
		if (mousePressDown)
			DrawButtonState(aContext, x, y, backColourActive, foreColourActive);

		else if (mouseOver)
			DrawButtonState(aContext, x, y, backColourHover, foreColourHover);

		else
			DrawButtonState(aContext, x, y, backColour, foreColour);


		// Draw background image if set
		if (backgroundImage != nullptr)
			OdDraw::ResourceImage(aContext, x + (padding/2), y + (padding/2), size.x - padding, size.y - padding, backgroundImage);


		OdDraw::RectStroke(aContext, x, y, size.x, size.y, 1, stroke);

		if (active)
			OdDraw::RectStroke(aContext, x + 1, y + 1, size.x - 2, size.y - 2, 0.6, OdColour::WHITE);

	}


	void OdButton::actionEvents(GrInputMap* aInput)
	{

	}


} // namespace OD::Graphics

#endif // !OD_GR_UI_BUTTON_CPP