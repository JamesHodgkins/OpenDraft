#ifndef OD_GR_VIEWPORT_CPP
#define OD_GR_VIEWPORT_CPP

/**************************************************************************************
* OpenDraft:    Main Graphics Header                                                  *
*-------------------------------------------------------------------------------------*
* Filename:     OdGraphics.cpp                                                        *
* Contributors: James Hodgkins                                                        *
* Date:         June 28, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
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

		cursorPosition = OdVector2(0, 0);

		backColour = OdColour::BACKGROUND1;
		stroke = OdColour(0, 0, 0, 0);
		foreColour = OdColour(255, 255, 255, 200);

		position = OdVector2(0, 0);

		text = "";
	}

	void OdViewport::onFrame(NVGcontext* aContext) 
	{
		// Static cast properties
		float x = getLocation().x;
		float y = getLocation().y;
		float w = size.x;
		float h = size.y;


		//
		// Begin background
		//
		OdDraw::Rect(aContext, x, y, w, h, backColour);
		OdDraw::RectStroke(aContext, x, y, w, h, 0.5, stroke);

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

		// Draw cursor
		if (mouseOver)
		{
			OdDraw::Line(aContext, 0, cursorPosition.y, size.x, cursorPosition.y, 0.8, OdColour::LIME);	// Horizontal
			OdDraw::Line(aContext, cursorPosition.x, 0, cursorPosition.x, size.y, 0.8, OdColour::RED);	// Vertical
		}

		// Undo translation
		OdDraw::Translate(aContext, -location.x, -location.y);

	}


	void OdViewport::actionEvents(GrInputMap* aInput)
	{
		// Check if mouse is over viewport
		if (aInput == nullptr)
			return;
		
		
		// Update cursor position
		cursorPosition.x = aInput->mouse.position.x - location.x;
		cursorPosition.y = aInput->mouse.position.y - location.y;


		//
		// Use mouse to move viewport position
		//

		// If shift is selected, add speed multiplier
		float multiplier = 3;

		if (aInput->keys[GLFW_KEY_LEFT_SHIFT].isDown())
		{
			OdSystem* system = OdSystem::getInstance();
			OdSystemVariable* multiplierVar = system->getRegistryVariableByName("viewport_speed_multiplier");
			
			if (multiplierVar != nullptr)
				multiplier = std::get<int>(multiplierVar->getValue());
			else
				multiplier = 10; // Default value if variable is not found
		}

		if (aInput->keys[GLFW_KEY_LEFT_CONTROL].isDown())
		{
			multiplier = 0.5;
		}

		if (aInput->keys[GLFW_KEY_LEFT].isDown())
			translatePosition(-1*multiplier, 0);

		if (aInput->keys[GLFW_KEY_RIGHT].isDown())
			translatePosition(1*multiplier, 0);

		if (aInput->keys[GLFW_KEY_UP].isDown())
			translatePosition(0, -1*multiplier);

		if (aInput->keys[GLFW_KEY_DOWN].isDown())
			translatePosition(0, 1*multiplier);


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
			scaleByAmount(0.01, aInput->mouse.position.x, aInput->mouse.position.y);

		if (aInput->mouse.scroll < 0)
			scaleByAmount(-0.01, aInput->mouse.position.x, aInput->mouse.position.y);

		OdVector2 worldMouse = getCoordinatesAtScreenPosition(aInput->mouse.position.x, aInput->mouse.position.y);
		updateEntityStates(worldMouse);
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

	void OdViewport::setScale(float aScale, int aMouseX, int aMouseY)
	{
		scale = aScale;
	}

	void OdViewport::scaleByAmount(float aAmount, int aMouseX, int aMouseY)
	{
		// TODO: Fix translation when scaling

		OdVector2 mB = OdVector2(aMouseX, aMouseY);
		auto mBw = getCoordinatesAtScreenPosition(mB.x, mB.y);
		

		float oldScale = scale;
		scale += aAmount;

		float deltaFactor = scale/oldScale;

		float deltaX = mB.x - (mB.x * deltaFactor);
		float deltaY = mB.y - (mB.y * deltaFactor);

		translatePosition(deltaX, -deltaY);
	}


	float OdViewport::getScale()
	{
		return scale;
	}


	OdVector2 OdViewport::getCoordinatesAtScreenPosition(float x, float y)
	{
		// Get inverse scale
		float invScale = 1.0f / scale;
		OdVector2 absoluteLocation = getOffset();

		// Get offset for viewport position
		OdVector2 result = OdVector2();
		result.x = ((x - absoluteLocation.x) * invScale) - position.x;
		result.y = ((y - absoluteLocation.y) * invScale) - position.y;
		return result;
	}

	OdVector2 OdViewport::getScreenPositionAtCoordinates(float worldX, float worldY)
	{
		// Get inverse scale
		float invScale = 1.0f / scale;
		OdVector2 absoluteLocation = getOffset();

		// Calculate screen coordinates from world coordinates
		OdVector2 result = OdVector2();
		result.x = (worldX + position.x) * scale + absoluteLocation.x;
		result.y = (worldY + position.y) * scale + absoluteLocation.y;
		return result;
	}

	void OdViewport::updateEntityStates(OdVector2 aPoint)
	{
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


		// Update entity states
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

			// Invoke entity hit test
			bool isHighlighted = entity->hitTest(aPoint, 5);
			entity->setHighlight(isHighlighted);
		}
	}

}

#endif // !OD_GR_GRAPHICS_CPP
