#ifndef OD_SY_LAYER_CPP
#define OD_SY_LAYER_CPP

/**************************************************************************************
* OpenDraft:    System Layer Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdLayer.cpp                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         July 10, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Entity layer class for OpenDraft framework.                                       *
***************************************************************************************/


#include "System/Objects/OdLayer.h"


using namespace OD::Data;


namespace OD::System
{

	OdLayer::OdLayer() :
		name(""),
		description(""),
		colour(7),
		lineWeight('0')
	{}
		
	OdLayer::OdLayer(const OdLayer& layer) :
		name(layer.name),
		description(layer.description),
		colour(layer.colour),
		lineWeight(layer.lineWeight)
	{}
		
	OdLayer::OdLayer(
		const std::string& aName,
		const std::string& aDescription,
		int aColour,
		const std::string& aLineType,
		int aLineWeight) :
			
		name(aName),
		description(aDescription),
		colour(aColour),
		lineWeight(aLineWeight)
	{}
		
	OdLayer::~OdLayer()	{}



	// Getters
	std::string OdLayer::getName() const
	{
		return name;
	}
			
	std::string OdLayer::getDescription() const
	{
		return description;
	}
			
	int OdLayer::getColor() const
	{
		return colour;
	}
			
	char OdLayer::getLineWeight() const
	{
		return lineWeight;
	}

	// Setters
	void OdLayer::setName(const std::string& aName)
	{
		name = aName;
	}
			
	void OdLayer::setDescription(const std::string & aDescription)
	{
		description = aDescription;
	}
			
	void OdLayer::setColor(const int aColour)
	{
		colour = aColour;
	}
			
	void OdLayer::setLineWeight(const char aLineWeight)
	{
		lineWeight = aLineWeight;
	}

	// Operators
	OdLayer& OdLayer::operator=(const OdLayer& aLayer)
	{
		if (this != &aLayer)
		{
			name = aLayer.name;
			description = aLayer.description;
			colour = aLayer.colour;
			lineWeight = aLayer.lineWeight;
		}
		return *this;
	}


	//
	// Base class overrides
	//

	// Get Type
	const char* OdLayer::getType() const
	{
		return "Layer";
	}

	// Serialise
	void OdLayer::serialise(std::wstring& buffer)
	{

	}

	void OdLayer::deserialise(std::wstring buffer)
	{

	}

} // namespace OD::System


#endif // !OD_SY_LAYER_CPP