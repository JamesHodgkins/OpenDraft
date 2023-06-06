#pragma once

#include "nanovg.h"
#include "Graphics/UI/GrUIComponent.h"

class GrUIButton : public GrUIComponent
{
public:

	GrColour backgroundColour = { 255,255,255 };

	GrUIButton(int x=0, int y=0, int width=150, int height=60)
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

		nvgFill(vg);
		nvgClosePath(vg);
	}



};