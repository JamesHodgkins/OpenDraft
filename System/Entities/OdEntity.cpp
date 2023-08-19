#ifndef OD_SY_ENTITY_CPP
#define OD_SY_ENTITY_CPP

/**************************************************************************************
* OpenDraft:    System Base Entity Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdEntity.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 30, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Base Entity class for OpenDraft framework.                                        *
***************************************************************************************/


#include "System/Entities/OdEntity.h"
#include "System/OdApplication.h"


using namespace OD::System;


namespace OD::Geometry
{
	// Getters & Setters
	OdVector2 OdEntity::getLocation() const { return location; }
	void OdEntity::setLocation(OdVector2 location) { this->location = location; }

	int OdEntity::getColour() const { return colour; }
	void OdEntity::setColour(int colour) { this->colour = colour; }
	OdColour OdEntity::getDrawColour() const
	{
		// Get colour
		OdColour drawColour;
		if (colour == -1)
			drawColour = OdColour::colourByIndex(layer->colour);

		else
			drawColour = OdColour::colourByIndex(colour);
		
		return drawColour;
	}

	int OdEntity::getLineType() const { return lineType; }
	void OdEntity::setLineType(int lineType) { this->lineType = lineType; }

	int OdEntity::getLineWeight() const { return lineWeight; }
	void OdEntity::setLineWeight(int lineWeight) { this->lineWeight = lineWeight; }
	float OdEntity::getDrawLineWeight() const
	{
		// Get line weight
		float activeLineWeight;
		if (lineWeight == -1)
			activeLineWeight = layer->lineWeight;

		else
			activeLineWeight = lineWeight;

		OdSystem* sys = OdSystem::getInstance();
		return sys->getDrawLineWeightByIndex(activeLineWeight);
	}

	OdLayer* OdEntity::getLayer() const { return layer; }
	void OdEntity::setLayer(OdLayer* layer) { this->layer = layer; }
	
	void OdEntity::setLayer(std::string aLayer)
	{
		// Get the layer from the application
		OdApplication* app = OdApplication::getInstance();
		OdDocument* doc = app->getDocumentManager()->getActiveDocument();
		OdDrawingDb* db = doc->getDatabase();
		
		for (int i = 0; i < db->getRecordCount(); i++)
		{
			OdDbObject* object = db->getRecordByIndex(i);

			// Check is object is not null
			if (object == nullptr) continue;

			// Check if object is a layer
			if (object->getType() == "Layer")
			{
				OdLayer* iLayer = dynamic_cast<OdLayer*>(object);
				if (iLayer->getName() == aLayer)
				{
					layer = iLayer;
					break;
				}
			}
		}
	}

	void OdEntity::setHighlight(bool aState)
	{
		highlight = aState;
	}

	bool OdEntity::isHighlighted() const
	{
		return highlight;
	}

	// Selection Methods
	void OdEntity::setSelected(bool aSelected)
	{
		selected = aSelected;
	}

	bool OdEntity::isSelected() const
	{
		return selected;
	}


} // namespace OD::Geometry

#endif // !OD_SY_ENTITY_CPP