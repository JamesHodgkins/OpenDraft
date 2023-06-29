#ifndef OD_SY_LAYER_CPP
#define OD_SY_LAYER_CPP

/**************************************************************************************
* OpenDraft:    System Layer Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdLayer.cpp                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         June 29, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   A class representing a layer in a drawing.                                        *
***************************************************************************************/


#include "System/Utils/OdLayer.h"


namespace OD::System
{
	// Constructor
	OdLayer::OdLayer(std::string aName)
	{
		name = aName;
	}

	// Getters and setters
	int OdLayer::getId() { return id; }

	// Colour index
	int OdLayer::getColourIndex() { return colourIndex; }
	void OdLayer::setColourIndex(int aColourIndex) { colourIndex = aColourIndex; }

	// Line weight index
	int OdLayer::getLineWeightIndex() { return lineWeightIndex; }
	void OdLayer::setLineWeightIndex(int aLineWeightIndex) { lineWeightIndex = aLineWeightIndex; }

	// Line type index
	int OdLayer::getLineTypeIndex() { return lineTypeIndex; }
	void OdLayer::setLineTypeIndex(int aLineTypeIndex) { lineTypeIndex = aLineTypeIndex; }

	// Name
	std::string OdLayer::getName() { return name; }
	void OdLayer::setName(std::string aName) { name = aName; }

}

#endif // OD_SY_LAYER_CPP
