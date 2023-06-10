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



#include "Graphics/UI/OdGrUiComponent.h"



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
	OdGrUiButton(int x=0, int y=0, int width=150, int height=30, std::string t="Button")
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

		nvgBeginPath(vg);
		nvgRect(vg, baseLocation.x, baseLocation.y, size.x, size.y);

		if (mouseOver)
			nvgFillColor(vg, backColourHover.asNvgColour() );
		else
			nvgFillColor(vg, backColour.asNvgColour());

		if (mouseDown) {
			nvgFillColor(vg, backColourActive.asNvgColour());
		}

		nvgFill(vg);
		nvgClosePath(vg);

		nvgBeginPath(vg);

		if (mouseOver)
			nvgFillColor(vg, foreColourHover.asNvgColour());
		else
			nvgFillColor(vg, foreColour.asNvgColour());

		if (mouseDown) {
			nvgFillColor(vg, foreColourActive.asNvgColour());
		}
		
		nvgFill(vg);
		nvgCreateFont(vg, "sans", ".\\OpenSans.ttf");
		nvgFontSize(vg, 14.0f);
		nvgFontFace(vg, "sans");
		nvgTextAlign(vg, NVG_ALIGN_CENTER | NVG_ALIGN_MIDDLE);
		nvgText(vg, baseLocation.x + size.x/2, baseLocation.y + size.y/2, text.c_str(), nullptr);

		nvgClosePath(vg);
	}



};