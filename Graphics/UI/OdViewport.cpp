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
		absoluteLocation = OdVector2(0, 0);

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
			entity->draw(aContext, &position, scale);
		}




		// Get the current transformation matrix
		float matrix[6];
		nvgCurrentTransform(aContext, matrix);

		absoluteLocation.x = matrix[4]; // + location.x;
		absoluteLocation.y = matrix[5]; // + location.y;


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

		// Was mouse pressed
		if (aInput->mouse.middleButton.isPressDown())
		{
			if (mouseOver)
			{
				dragState = true;
				dragStart = OdVector2(aInput->mouse.position.x, aInput->mouse.position.y);
			}
		}
		
		if (aInput->mouse.middleButton.isDown())	// Use mouse to move viewport position
		{
			// if dragState is false, set dragState to true and set dragStart to mouse position
			if (dragState)
 			{
				float newX = (aInput->mouse.position.x - dragStart.x) / scale;
				float newY = (aInput->mouse.position.y - dragStart.y) / scale;
				translatePosition(newX, newY);

 				dragStart = OdVector2(aInput->mouse.position.x, aInput->mouse.position.y);
 			}
		}
		else	// If middle button is not down, set dragState to false
		{
			dragState = false;
		}

		// Scale viewport
		if (aInput->mouse.scroll > 0)
			scaleIn();

		if (aInput->mouse.scroll < 0)
			scaleOut();
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

	void OdViewport::setScale(float aScale)
	{
		scale = aScale;
	}

	void OdViewport::scaleIn()
	{
		scale += 0.1f;
	}

	void OdViewport::scaleOut()
	{
		scale -= 0.1f;
	}

	float OdViewport::getScale()
	{
		return scale;
	}


	OdVector2 OdViewport::getCoordinatesAtScreenPosition(float x, float y)
	{
		// Get inverse scale
		float invScale = 1.0f / scale;

		// Get offset for viewport position
		OdVector2 result = OdVector2();
		result.x = (x - absoluteLocation.x) * invScale - (position.x);
		result.y = (y - absoluteLocation.y) * invScale - (position.y);
		return result;
	}

}

#endif // OD_GR_GRAPHICS_CPP
