#ifndef OD_SY_ENTITY_H
#define OD_SY_ENTITY_H

/**************************************************************************************
* OpenDraft:    System Base Entity Class                                              *
*-------------------------------------------------------------------------------------*
* Filename:     OdEntity.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Base Entity class for OpenDraft framework.                                        *
***************************************************************************************/


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

		protected:
			void setHandle(unsigned int handle) { this->handle = handle; }

		public:
			OdVector2 location;

			// Constructors & Destructors
			virtual ~OdEntity() = default;

			// Getters & Setters
			int getHandle() const { return handle; }
			int getParent() const { return parent; }
			void setParent(int parent) { this->parent = parent; }
			OdVector2 getLocation() const { return location; }
			void setLocation(OdVector2 location) { this->location = location; }

			// Methods
			virtual void draw(NVGcontext* aContext) = 0;
			
		};

	} // namespace Geometry
} // namespace OD

#endif // OD_SY_ENTITY_H