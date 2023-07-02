#ifndef OD_GR_VIEWPORT_CPP
#define OD_GR_VIEWPORT_CPP

/**************************************************************************************
* OpenDraft:    Main Graphics Header                                                  *
*-------------------------------------------------------------------------------------*
* Filename:     OdGraphics.cpp                                                        *
* Contributors: James Hodgkins                                                        *
* Date:         June 28, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Header file collates all necessary graphics                                       *
***************************************************************************************/



#include "Graphics/UI/OdViewport.h"


using namespace OD::Geometry;

namespace OD::Graphics
{
	
	// Constructor
	OdViewport::OdViewport(int aX, int aY, int aWidth, int aHeight)
	{
		location.x = aX;
		location.y = aY;
		size.x = aWidth;
		size.y = aHeight;

		backColour = OdColour::BACKGROUND1;
		stroke = OdColour(0, 0, 0, 0);
		foreColour = OdColour(255, 255, 255, 200);

		text = "";
	}

	void OdViewport::onFrame(NVGcontext* aContext) 
	{
		// Check if enabled
		if (!enabled)
			return;

		// Check if context is null
		if (aContext == nullptr)
			return;

		// Static cast properties
		float x = getLocation().x;
		float y = getLocation().y;
		float w = size.x;
		float h = size.y;


		//
		// Begin background
		//
		OdDraw::Rect(aContext, x, y, w, h, backColour);
		OdDraw::RectStroke(aContext, x, y, w, h, stroke);

		disableOverflow(aContext);

		// Check is entities is valid
		if (entities == nullptr)
			return;

		// Translate by location
		OdDraw::Translate(aContext, location.x, location.y);

		// Draw entities
		for (OdEntity* entity : *entities)
			entity->draw(aContext);
		
		// Undo translation
		OdDraw::Translate(aContext, -location.x, -location.y);

	}

}

#endif // OD_GR_GRAPHICS_CPP
