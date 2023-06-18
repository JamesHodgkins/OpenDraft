#pragma once

/**************************************************************************************
* OpenDraft:    Graphic Draw Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdGrDraw.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 15, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Draw routine wrappers for NanoVG intended for GUI rendering                       *
***************************************************************************************/



#include "Libraries/nanovg/src/nanovg.h"
#include "System/OdSyCore.h"
#include <iostream>



class OdGrDraw
{
public:

	// Draws a filled rectangle on the specified NanoVG context.
	static void Rect(NVGcontext* context, int x, int y, int width, int height, OdSyColour colour);

	// Draws a stroked rectangle on the specified NanoVG context.
	static void RectStroke(NVGcontext* context, int x, int y, int width, int height, OdSyColour colour);

	// Draws text on the specified NanoVG context.
	static void Text(NVGcontext* context, int x, int y, int width, int height, float size, OdSyColour colour, const char* text);

	// Draws an image on the specified NanoVG context from a file path.
	static void ResourceImage(NVGcontext* context, int x, int y, int width, int height, OdSyImage* img);
	static void ResourceImage(NVGcontext* context, int x, int y, OdSyImage* img);

};




void OdGrDraw::Rect(NVGcontext* context, int x, int y, int width, int height, OdSyColour colour)
{
	nvgBeginPath(context);
	nvgRect(context, x, y, width, height);
	nvgFillColor(context, colour.asNvgColour());
	nvgFill(context);
	nvgClosePath(context);
}


void OdGrDraw::RectStroke(NVGcontext* context, int x, int y, int width, int height, OdSyColour colour)
{
	nvgBeginPath(context);
	nvgRect(context, x, y, width, height);
	nvgStrokeColor(context, colour.asNvgColour());
	nvgStroke(context);
	nvgClosePath(context);
}


void OdGrDraw::Text(NVGcontext* context, int x, int y, int width, int height, float size, OdSyColour colour, const char* text)
{
	nvgBeginPath(context);
	nvgFillColor(context, colour.asNvgColour());
	nvgFill(context);
	nvgFontSize(context, size);
	nvgFontFace(context, "sans");
	nvgTextAlign(context, NVG_ALIGN_CENTER | NVG_ALIGN_MIDDLE);
	nvgText(context, x + width / 2, y + height / 2, text, nullptr);
	nvgClosePath(context);
}


void OdGrDraw::ResourceImage(NVGcontext* context, int x, int y, int width, int height, OdSyImage* img)
{
	if (img == nullptr)
		return;

	if (img->getId() != 0)
	{
		nvgBeginPath(context);
		nvgRect(context, x, y, img->getWidth(), img->getHeight());
		nvgFillPaint(context, nvgImagePattern(context, x, y, width, height, 0, img->getId(), 1.0f));
		nvgFill(context);
		nvgClosePath(context);
		//nvgDeleteImage(context, imageHandle); // To do link delete image mem to deleting image class
	}
}

void OdGrDraw::ResourceImage(NVGcontext* context, int x, int y, OdSyImage* img)
{
	if (img == nullptr)
		return;

	OdGrDraw::ResourceImage(context, x, y, img->getWidth(), img->getHeight(), img);
}