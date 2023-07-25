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
		backColour = OdColour::WHITE;
	}

	OdTextbox::OdTextbox(int aX, int aY, int aWidth, int aHeight, std::string aText)
	{
		location.x = aX;
		location.y = aY;
		size.x = aWidth;
		size.y = aHeight;
		foreColour = OdColour::BLACK;
		backColour = OdColour::WHITE;
		text = aText;
		cursorIndex = 0;
		cursorBlinkTimer = 0;
	}

	OdTextbox::OdTextbox(OdVector2 aLocation, std::string aText)
	{
		location.x = aLocation.x;
		location.y = aLocation.y;
		foreColour = OdColour::BLACK;
		backColour = OdColour::WHITE;
		text = aText;
		cursorIndex = 0;
		cursorBlinkTimer = 0;
	}



	int OdTextbox::calculateCursorPosition(NVGcontext* aContext, int aIndex)
	{
		// Use nanovg to calculate text width
		float bounds[4];

		// Static cast properties
		float textboxX = getWidth();
		float textboxY = getHeight();

		// Get characters of text property up to aIndex
		std::string subText = text.substr(0, aIndex);

		nvgReset(aContext);
		nvgTextBounds(aContext, textboxX, textboxY, subText.c_str(), nullptr, bounds);


		return (int)(bounds[2]-bounds[0]);

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

		OdDraw::TextStyle textboxTextStyle =
		{
			14,
			"sans",
			foreColour,
			OdDraw::Alignment::Left
		};

		OdDraw::Rect(aContext, x, y, w, h, backColour);
		OdDraw::Text(aContext, x - (w/2), y +2, w, h, textboxTextStyle, text.c_str());

		// if cursorBlinkTimer is less than half the time, draw the cursor
		if (cursorBlinkTimer < CURSOR_BLINK_FRAME_COUNT)
		{
			int cursorX = calculateCursorPosition(aContext, cursorIndex) + x;
			OdDraw::Line(aContext, cursorX, y, cursorX, y + h, 2, OdColour::RED);
		}

		// Decrement cursor timer
		cursorBlinkTimer--;
	}



	void OdTextbox::actionEvents(GrInputMap* aInput)
	{
		for (int key = GLFW_KEY_0; key <= GLFW_KEY_9; key++) {
			if (aInput->keys[key].isPressDown())
			{
				// Convert the GLFW keycode to its corresponding character using ASCII values
				char character = static_cast<char>(key);
				text += character;
				cursorIndex++;
			}
		}

		for (int key = GLFW_KEY_A; key <= GLFW_KEY_Z; key++) {
			if (aInput->keys[key].isPressDown())
			{
				if (aInput->keys[GLFW_KEY_LEFT_SHIFT].isDown())
				{
					// Convert the GLFW keycode to its corresponding character using ASCII values
					char character = static_cast<char>(key);
					text += character;
					cursorIndex++;
				}
				else
				{
					// Convert the GLFW keycode to its corresponding character using ASCII values
					char character = static_cast<char>(key + 32);
					text += character;
					cursorIndex++;
				}
			}
		}

		if (aInput->keys[GLFW_KEY_SPACE].isPressDown())
		{
			text += " ";
			cursorIndex++;
		}

		// Backspace
		if (aInput->keys[GLFW_KEY_BACKSPACE].isPressDown())
		{
			if (text.length() > 0)
			{
				text.pop_back();
				cursorIndex--;
			}
		}

	}



}

#endif // !OD_GR_UI_TEXTBOX_CPP