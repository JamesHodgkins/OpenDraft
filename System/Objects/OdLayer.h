#ifndef OD_SY_LAYER_H
#define OD_SY_LAYER_H

/**************************************************************************************
* OpenDraft:    System Layer Class                                                    *
*-------------------------------------------------------------------------------------*
* Filename:     OdLayer.h                                                             *
* Contributors: James Hodgkins                                                        *
* Date:         July 10, 2023                                                         *
* Copyright:    ©2023 OpenDraft. GPLv3                                                *
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
			int colour;
			//std::string lineType;
			char lineWeight;


			// Constructors and Destructors
			OdLayer();
			OdLayer(const OdLayer& layer);
			OdLayer(const std::string& name, const std::string& description, const int colour, const std::string& lineType, int lineWeight);
			~OdLayer();


			// Getters and Setters
			std::string getName() const;
			void setName(const std::string& aName);
			
			std::string getDescription() const;
			void setDescription(const std::string& aDescription);
			
			int getColor() const;
			void setColor(const int aColor);
			
			//std::string getLineType() const;
			//void setLineType(const std::string& lineType);
			
			char getLineWeight() const;
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



#endif // !OD_SY_LAYER_H