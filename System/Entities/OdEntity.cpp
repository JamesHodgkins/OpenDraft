#ifndef OD_SY_ENTITY_CPP
#define OD_SY_ENTITY_CPP

/**************************************************************************************
* OpenDraft:    System Base Entity Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdEntity.cpp                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 30, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
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

	int OdEntity::getLineType() const { return lineType; }
	void OdEntity::setLineType(int lineType) { this->lineType = lineType; }

	int OdEntity::getLineWeight() const { return lineWeight; }
	void OdEntity::setLineWeight(int lineWeight) { this->lineWeight = lineWeight; }

	OdLayer* OdEntity::getLayer() const { return layer; }
	void OdEntity::setLayer(OdLayer* layer) { this->layer = layer; }


} // namespace OD::Geometry

#endif // OD_SY_ENTITY_CPP