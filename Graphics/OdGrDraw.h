#pragma once

/**************************************************************************************
* OpenDraft:    Graphic Draw Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdGrDraw.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 11, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Draw routine wrappers for NanoVG intended for GUI rendering                       *
***************************************************************************************/



#include "Libraries/nanovg/src/nanovg.h"



class OdGrDraw
{
public:

	static void Rect(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour);
	static void RectStroke(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour);
	static void Text(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour, const char* text);

	// TEMP
	static void LoadFont(NVGcontext* vg);
};



void OdGrDraw::LoadFont(NVGcontext* vg)
{
	nvgCreateFont(vg, "sans", ".\\OpenSans.ttf");
}


void OdGrDraw::Rect(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour)
{
	nvgBeginPath(vg);
	nvgRect(vg, x, y, width, height);
	nvgFillColor(vg, colour.asNvgColour());
	nvgFill(vg);
	nvgClosePath(vg);
}

void OdGrDraw::RectStroke(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour)
{
	nvgBeginPath(vg);
	nvgRect(vg, x, y, width, height);
	nvgStrokeColor(vg, colour.asNvgColour());
	nvgStroke(vg);
	nvgClosePath(vg);
}


void OdGrDraw::Text(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour, const char* text)
{
	nvgBeginPath(vg);
	nvgFillColor(vg, colour.asNvgColour());
	nvgFill(vg);
	nvgFontSize(vg, 14.0f);
	nvgFontFace(vg, "sans");
	nvgTextAlign(vg, NVG_ALIGN_CENTER | NVG_ALIGN_MIDDLE);
	nvgText(vg, x + width / 2, y + height / 2, text, nullptr);
	nvgClosePath(vg);
}
