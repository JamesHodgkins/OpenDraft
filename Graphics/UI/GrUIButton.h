#pragma once

#include "nanovg.h"
#include "Graphics/UI/GrUIComponent.h"

class GrUIButton : public GrUIComponent
{
public:

	GrColour backgroundColour = { 255,255,255 };

	GrUIButton(int x=0, int y=0, int width=150, int height=30)
	{
		location.x = x;
		location.y = y;
		size.x = width;
		size.y = height;
	}

	virtual void const render(NVGcontext* vg) override
	{
		nvgBeginPath(vg);
		nvgRect(vg, location.x, location.y, size.x, size.y);

		if (mouseOver)
			nvgFillColor(vg, nvgRGBA(255, 0, 0, 255));
		else
			nvgFillColor(vg, nvgRGBA(0, 255, 0, 255));

		if (mouseDown) {
			nvgFillColor(vg, nvgRGBA(0, 0, 255, 255));
		}

		nvgFill(vg);
		nvgClosePath(vg);

		nvgBeginPath(vg);

		nvgFillColor(vg, nvgRGBA(0, 0, 0, 255));
		nvgFill(vg);
		nvgCreateFont(vg, "sans", ".\\OpenSans.ttf");
		nvgFontSize(vg, 14.0f);
		nvgFontFace(vg, "sans");
		nvgTextAlign(vg, NVG_ALIGN_CENTER | NVG_ALIGN_MIDDLE);
		nvgText(vg, location.x + size.x/2, location.y + size.y/2, "Hello World", nullptr);

		nvgClosePath(vg);
	}



};