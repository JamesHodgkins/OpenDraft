#ifndef OD_SY_LAYER_CPP
#define OD_SY_LAYER_CPP

/**************************************************************************************
* OpenDraft:    System Layer Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdLayer.cpp                                                           *
* Contributors: James Hodgkins                                                        *
* Date:         July 10, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Entity layer class for OpenDraft framework.                                       *
***************************************************************************************/


#include "System/Objects/OdLayer.h"


using namespace OD::Data;


namespace OD::System
	{

		OdLayer::OdLayer()
		{
			name = "";
			description = "";
			colour = OdColour();
			lineWeight = '0';
		}
		
		OdLayer::OdLayer(const OdLayer& layer)
		{
			name = layer.name;
			description = layer.description;
			colour = layer.colour;
			lineWeight = layer.lineWeight;
		}
		
		OdLayer::OdLayer(
			const std::string& name,
			const std::string& description,
			const std::string& colour,
			const std::string& lineType,
			const std::string& lineWeight,
			const std::string& plotStyle){}
		
		OdLayer::~OdLayer()
		{
			name = "";
			description = "";
			colour = OdColour();
			lineWeight = '0';
		}



		// Getters
		std::string OdLayer::getName() const
		{
			return name;
		}
			
		std::string OdLayer::getDescription() const
		{
			return description;
		}
			
		OdColour OdLayer::getColor() const
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
			
		void OdLayer::setColor(const OdColour & aColour)
		{
			colour.setRed(aColour.getRed());
			colour.setGreen(aColour.getGreen());
			colour.setBlue(aColour.getBlue());
			colour.setAlpha(aColour.getAlpha());
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

} // namespace OD::System


#endif // OD_SY_LAYER_CPP