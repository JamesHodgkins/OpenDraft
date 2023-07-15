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
#include "System/OdApplication.h"
#include "DataManager/OdData.h"
#include "Graphics/UI/OdInput.h"



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

		position = OdVector2(0, 0);

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

		// Translate by location
		OdDraw::Translate(aContext, location.x, location.y);

		// Get active document
		OdDocument* doc = OdApplication::getInstance()->getDocumentManager()->getActiveDocument();

		// Check if document is null
		if (doc == nullptr)
			return;

		OdDrawingDb* db = doc->getDatabase();

		// Check if database is null
		if (db == nullptr)
			return;

		// Number of entities
		int numEntities = db->getRecordCount();


		// Draw entities
		for (int i = 0; i < numEntities; i++)
		{
			// Get entity
			OdDbObject* dbObject = db->getRecordByIndex(i);

			// Check if entity is null
			if (dbObject == nullptr)
				continue;

			// Can item be cast as an OdEntity
			OdEntity* entity = dynamic_cast<OdEntity*>(dbObject);

			// Check if entity is null
			if (entity == nullptr)
				continue;

			// Draw entity
			entity->draw(aContext, &position);
		}
		
		// Undo translation
		OdDraw::Translate(aContext, -location.x, -location.y);

	}


	void OdViewport::actionEvents(GrInputMap* aInput)
	{
		if (aInput == nullptr)
			return;


		// Use mouse to move viewport position
		if (aInput->keys[GLFW_KEY_LEFT].isDown())
			translatePosition(-2, 0);

		if (aInput->keys[GLFW_KEY_RIGHT].isDown())
			translatePosition(2, 0);

		if (aInput->keys[GLFW_KEY_UP].isDown())
			translatePosition(0, -1);

		if (aInput->keys[GLFW_KEY_DOWN].isDown())
			translatePosition(0, 1);

		// Use mouse to move viewport position
		if (aInput->mouse.middleButton.isDown())
		{
			// if dragState is false, set dragState to true and set dragStart to mouse position
			if (!dragState)
			{
				dragState = true;
				dragStart = OdVector2(aInput->mouse.position.x, aInput->mouse.position.y);
			}

			// If dragState is true, translate position by difference between dragStart and mouse position
 			else
 			{
				translatePosition(aInput->mouse.position.x - dragStart.x, aInput->mouse.position.y - dragStart.y);
 				dragStart = OdVector2(aInput->mouse.position.x, aInput->mouse.position.y);
 			}
		}

		// If middle button is not down, set dragState to false
		else
			dragState = false;

	}


	void OdViewport::translatePosition(float aX, float aY)
	{
		position.x += aX;
		position.y += aY;
	}

	void OdViewport::setPosition(float aX, float aY)
	{
		position.x = aX;
		position.y = aY;
	}

	OdVector2 OdViewport::getPosition()
	{
		return position;
	}

}

#endif // OD_GR_GRAPHICS_CPP
