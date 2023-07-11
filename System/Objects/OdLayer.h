#ifndef OD_SY_LAYER_H
#define OD_SY_LAYER_H

/**************************************************************************************
* OpenDraft:    System Layer Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdLayer.h                                                             *
* Contributors: James Hodgkins                                                        *
* Date:         July 10, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Entity layer class for OpenDraft framework.                                       *
***************************************************************************************/


#include "DataManager/OdData.h"


using namespace OD::Data;


namespace OD
{
	namespace System
	{

		class OdLayer : public OdDbObject
		{

		public:

			// Properties
			std::string name;
			std::string description;
			OdColour colour;
			//std::string lineType;
			char lineWeight;


			OdLayer();
			OdLayer(const OdLayer& layer);
			OdLayer(const std::string& name, const std::string& description, const OdColour colour, const std::string& lineType, int lineWeight);
			~OdLayer();

			// Getters
			std::string getName() const;
			std::string getDescription() const;
			OdColour getColor() const;
			//std::string getLineType() const;
			char getLineWeight() const;

			// Setters
			void setName(const std::string& aName);
			void setDescription(const std::string& aDescription);
			void setColor(const OdColour& aColor);
			//void setLineType(const std::string& lineType);
			void setLineWeight(const char aLineWeight);

			// Operators
			OdLayer& operator=(const OdLayer& aLayer);



			//
			// Base class overrides
			//

			// Get Type
			virtual const char* getType() const override;

			// Serialise
			virtual void serialise(std::wstring& buffer) override;
			virtual void deserialise(std::wstring buffer) override;

		};

	}
}



#endif // OD_SY_LAYER_H