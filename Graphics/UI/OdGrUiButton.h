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

	OdGrUiButton(int x=0, int y=0, int width=150, int height=30)
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
	}

	virtual void const onFrame(NVGcontext* vg) override
	{
		nvgBeginPath(vg);
		nvgRect(vg, location.x, location.y, size.x, size.y);

		if (mouseOver)
			nvgFillColor(vg, backColourHover.asNVG() );
		else
			nvgFillColor(vg, backColour.asNVG());

		if (mouseDown) {
			nvgFillColor(vg, backColourActive.asNVG());
		}

		nvgFill(vg);
		nvgClosePath(vg);

		nvgBeginPath(vg);

		if (mouseOver)
			nvgFillColor(vg, foreColourHover.asNVG());
		else
			nvgFillColor(vg, foreColour.asNVG());

		if (mouseDown) {
			nvgFillColor(vg, foreColourActive.asNVG());
		}
		
		nvgFill(vg);
		nvgCreateFont(vg, "sans", ".\\OpenSans.ttf");
		nvgFontSize(vg, 14.0f);
		nvgFontFace(vg, "sans");
		nvgTextAlign(vg, NVG_ALIGN_CENTER | NVG_ALIGN_MIDDLE);
		nvgText(vg, location.x + size.x/2, location.y + size.y/2, text.c_str(), nullptr);

		nvgClosePath(vg);
	}



};