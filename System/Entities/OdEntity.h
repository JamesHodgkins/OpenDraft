#ifndef OD_SY_ENTITY_H
#define OD_SY_ENTITY_H

/**************************************************************************************
* OpenDraft:    System Base Entity Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdEntity.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 30, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Base Entity class for OpenDraft framework.                                        *
***************************************************************************************/


#include <fstream>
#include "System/OdCore.h"
#include "DataManager/OdDbObject.h"
#include "System/Objects/OdLayer.h"


using namespace OD::System;
using namespace OD::Data;


namespace OD
{
	namespace Geometry
	{

		class OdEntity : public OdDbObject {

		private:

			int colour = 0;
			int lineType = 0;
			int lineWeight = 0;
			OdLayer* layer = nullptr;

		protected:


		public:
			OdVector2 location;

			// Constructors & Destructors
			OdEntity() : OdDbObject() {}
			virtual ~OdEntity() = default;

			// Getters & Setters
			OdVector2 getLocation() const;
			void setLocation(OdVector2 location);

			int getColour() const;
			void setColour(int colour);

			int getLineType() const;
			void setLineType(int lineType);

			int getLineWeight() const;
			void setLineWeight(int lineWeight);

			OdLayer* getLayer() const;
			void setLayer(OdLayer* layer);

			// Methods
			virtual void draw(NVGcontext* aContext) = 0;
			
		};

	} // namespace OD::Geometry
} // namespace OD

#endif // OD_SY_ENTITY_H