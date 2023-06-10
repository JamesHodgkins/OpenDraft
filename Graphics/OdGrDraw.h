#pragma once


#include "Libraries/nanovg/src/nanovg.h"


class OdGrDraw
{
public:

	static void Rect(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour);
	static void Text(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour, const char* text);
};


void OdGrDraw::Rect(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour)
{
	nvgBeginPath(vg);
	nvgRect(vg, x, y, width, height);
	nvgFillColor(vg, colour.asNvgColour());
	nvgFill(vg);
	nvgClosePath(vg);
}


void OdGrDraw::Text(NVGcontext* vg, int x, int y, int width, int height, OdSyColour colour, const char* text)
{
	nvgBeginPath(vg);
	nvgFillColor(vg, colour.asNvgColour());
	nvgFill(vg);
	nvgCreateFont(vg, "sans", ".\\OpenSans.ttf");
	nvgFontSize(vg, 14.0f);
	nvgFontFace(vg, "sans");
	nvgTextAlign(vg, NVG_ALIGN_CENTER | NVG_ALIGN_MIDDLE);
	nvgText(vg, x + width / 2, y + height / 2, text, nullptr);
	nvgClosePath(vg);
}
