#ifndef OD_GR_UI_TEXTBOX_CPP
#define OD_GR_UI_TEXTBOX_CPP

/**************************************************************************************
* OpenDraft:    GUI Textbox Class                                                     *
*-------------------------------------------------------------------------------------*
* Filename:     OdTextbox.cpp                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         July 24, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI textbox input class derived from OdComponent                                *
***************************************************************************************/



#include "Graphics/UI/OdTextbox.h"


namespace OD::Graphics
{
	
	OdTextbox::OdTextbox()
	{
		text = "";
		cursorIndex = 0;
		cursorBlinkTimer = 0;
		foreColour = OdColour::BLACK;
	}

	OdTextbox::OdTextbox(int aX, int aY, int aWidth, int aHeight, std::string aText)
	{
		location.x = aX;
		location.y = aY;
		size.x = aWidth;
		size.y = aHeight;
		foreColour = OdColour::BLACK;
		text = aText;
		cursorIndex = 0;
		cursorBlinkTimer = 0;
	}

	OdTextbox::OdTextbox(OdVector2 aLocation, std::string aText)
	{
		location.x = aLocation.x;
		location.y = aLocation.y;
		foreColour = OdColour::BLACK;
		text = aText;
		cursorIndex = 0;
		cursorBlinkTimer = 0;
	}



	int OdTextbox::calculateCursorPosition()
	{
		return 20;
	}


	void OdTextbox::onFrame(NVGcontext* aContext)
	{
		// If cursor timer is 0, reset it. Multiply by 2, half time is on and half off
		if (cursorBlinkTimer < 1)
			cursorBlinkTimer = CURSOR_BLINK_FRAME_COUNT*2;


		// Static cast parameters to int
		int w = static_cast<int>(size.x);
		int h = static_cast<int>(size.y);
		int x = static_cast<int>(location.x);
		int y = static_cast<int>(location.y);

		OdDraw::Rect(aContext, x, y, w, h, backColour);
		OdDraw::Text(aContext, x, y, w, h, fontSize, foreColour, text.c_str());

		// if cursorBlinkTimer is less than half the time, draw the cursor
		if (cursorBlinkTimer < CURSOR_BLINK_FRAME_COUNT)
		{
			int cursorX = calculateCursorPosition() + x;
			OdDraw::Line(aContext, cursorX, y, cursorX, y + h, 1, OdColour::RED);
		}

		// Decrement cursor timer
		cursorBlinkTimer--;
	}



	void OdTextbox::actionEvents(GrInputMap* aInput)
	{

	}



}

#endif // !OD_GR_UI_TEXTBOX_CPP