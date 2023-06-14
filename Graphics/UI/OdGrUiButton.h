#pragma once

/**************************************************************************************
* OpenDraft:    GUI Button Class                                                      *
*-------------------------------------------------------------------------------------*
* Filename:     OdGrUiButton.h                                                        *
* Contributors: James Hodgkins                                                        *
* Date:         June 9, 2023                                                          *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A GUI button class derived by OdGrComponent                                       *
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

public:

	/**
	 * \brief Constructs a Button object with the specified attributes.
	 * \param location (OdSyPoint) The position of the button.
	 * \param size (OdSyPoint) The size of the button.
	 * \param text (std::string) The text displayed on the button.
	 */
	OdGrUiButton(int x = 0, int y = 0, int width = 150, int height = 30, std::string t = "Button")
	{
		location.x = x;
		location.y = y;
		size.x = width;
		size.y = height;

		backColour = OdSyColour(255, 0, 0, 255);
		backColourHover = OdSyColour(0, 255, 0, 255);
		backColourActive = OdSyColour(0, 0, 255, 255);

		foreColour = OdSyColour(255, 255, 255, 200);
		foreColourHover = OdSyColour(25, 25, 25, 255);
		foreColourActive = OdSyColour(255, 255, 25, 255);

		text = t;
	}


	/**
	 * \brief Renders a default OD-GUI Button to a given NanoVG context (NVGContext) with the specified attributes.
	 * \param vg (NVGcontext*) The nanovg pointer for rendering.
	 */
	virtual void const onFrame(NVGcontext* vg) override
	{
		if (vg == nullptr)
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
			OdGrDraw::Rect(vg, baseLocation.x, baseLocation.y, size.x, size.y, backColourActive);
			OdGrDraw::Text(vg, baseLocation.x, baseLocation.y, size.x, size.y, foreColourActive, text.c_str());
		}
		else
		{
			if (mouseOver)
			{
				OdGrDraw::Rect(vg, baseLocation.x, baseLocation.y, size.x, size.y, backColourHover);
				OdGrDraw::Text(vg, baseLocation.x, baseLocation.y, size.x, size.y, foreColourHover, text.c_str());
			}
			else
			{
				OdGrDraw::Rect(vg, baseLocation.x, baseLocation.y, size.x, size.y, backColour);
				OdGrDraw::Text(vg, baseLocation.x, baseLocation.y, size.x, size.y, foreColour, text.c_str());
			}
		}

		OdGrDraw::RectStroke(vg, baseLocation.x, baseLocation.y, size.x, size.y, OdSyColour(255,255,255,255));

	
		
	}

};