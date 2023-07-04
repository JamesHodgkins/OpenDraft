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


using namespace OD::System;


namespace OD
{
	namespace Geometry
	{

		class OdEntity {
		private:

			unsigned int handle;
			unsigned int parent = 0; // 0 = drawing root

			int colour = 0;
			int lineType = 0;
			int lineWeight = 0;
			OdLayer* layer = nullptr;

		protected:
			void setHandle(unsigned int handle);

		public:
			OdVector2 location;

			// Constructors & Destructors
			virtual ~OdEntity() = default;

			// Getters & Setters
			int getHandle() const;
			int getParent() const;
			void setParent(int parent);
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

			// Serialise
			//virtual void serialise(std::ostream& aStream) = 0;
			//virtual void deserialise(std::istream& aStream) = 0;
			
		};

	} // namespace OD::Geometry
} // namespace OD

#endif // OD_SY_ENTITY_H