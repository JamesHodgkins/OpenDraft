#ifndef OD_GR_UI_BUTTON_H
#define OD_GR_UI_BUTTON_H

/**************************************************************************************
* OpenDraft:    GUI Button Class                                                      *
*-------------------------------------------------------------------------------------*
* Filename:     OdGrUiButton.h                                                        *
* Contributors: James Hodgkins                                                        *
* Date:         June 9, 2023                                                          *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI button class derived from OdGrComponent                                     *
***************************************************************************************/



#include "System/OdSyCore.h"
#include "Graphics/UI/OdGrUiComponent.h"
#include "Graphics/OdGrDraw.h"


class OdGrUiButton : public OdGrUiComponent
{
protected:
	OdSyColour backColourHover;
	OdSyColour foreColourHover;
	OdSyColour backColourActive;
	OdSyColour foreColourActive;
	OdSyImage* backgroundImage;

public:

	/**
	 * \brief Constructs a Button object with the specified attributes.
	 * \param location (OdSyPoint) The position of the button.
	 * \param size (OdSyPoint) The size of the button.
	 * \param text (std::string) The text displayed on the button.
	 */
	OdGrUiButton(int aX = 0, int aY = 0, int aWidth = 150, int aHeight = 30, std::string aText = "Button")
	{
		location.x = aX;
		location.y = aY;
		size.x = aWidth;
		size.y = aHeight;

		backColour = OdSyColour::BACKGROUND1;
		backColourHover = OdSyColour::BACKGROUND2;
		backColourActive = OdSyColour(0, 0, 255, 255);

		foreColour = OdSyColour(255, 255, 255, 200);
		foreColourHover = OdSyColour(25, 25, 25, 255);
		foreColourActive = OdSyColour(255, 255, 25, 255);

		stroke = OdSyColour(0, 0, 0, 0);

		backgroundImage = nullptr;

		text = aText;
	}

	void setBackgroundImage(OdSyImage* aImage)
	{
		backgroundImage = aImage;
	}

	void clearBackgroundImage()
	{
		backgroundImage = nullptr;
	}


	/**
	 * \brief Renders a default OD-GUI Button to a given NanoVG context (NVGContext) with the specified attributes.
	 * \param context (NVGcontext*) The nanovg pointer for rendering.
	 */
	virtual void const onFrame(NVGcontext* aContext) override
	{
		if (!enabled)
			return;

		if (aContext == nullptr)
			return;

		// Convert class location (relative to parent) to context location (relative to the window)
		OdSyPoint baseLocation;

		if (parent == nullptr) {
			baseLocation = OdSyPoint(location.x, location.y);
		}
		else
		{
			baseLocation = OdSyPoint(parent->getLocationInContext().x + location.x, parent->getLocationInContext().y + location.y);
		}

		//
		// Begin drawing Button
		//
		if (mouseDown)
		{
			OdGrDraw::Rect(aContext, baseLocation.x, baseLocation.y, size.x, size.y, backColourActive);
			OdGrDraw::Text(aContext, baseLocation.x, baseLocation.y, size.x, size.y, 14.0f, foreColourActive, text.c_str());
		}
		else
		{
			if (mouseOver)
			{
				OdGrDraw::Rect(aContext, baseLocation.x, baseLocation.y, size.x, size.y, backColourHover);
				OdGrDraw::Text(aContext, baseLocation.x, baseLocation.y, size.x, size.y, 14.0f, foreColourHover, text.c_str());
			}
			else
			{
				OdGrDraw::Rect(aContext, baseLocation.x, baseLocation.y, size.x, size.y, backColour);
				OdGrDraw::Text(aContext, baseLocation.x, baseLocation.y, size.x, size.y, 14.0f, foreColour, text.c_str());
			}
		}


		if (backgroundImage != nullptr)
		{
			OdGrDraw::ResourceImage(aContext, baseLocation.x, baseLocation.y, size.x, size.y, backgroundImage);
		}

		OdGrDraw::RectStroke(aContext, baseLocation.x, baseLocation.y, size.x, size.y, stroke);

	}

};

#endif // OD_GR_UI_BUTTON_H