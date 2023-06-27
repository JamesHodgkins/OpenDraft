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

		public:

			virtual ~OdEntity() = default;

			OdVector2 location;
		};

	} // namespace Geometry
} // namespace OD

#endif // OD_SY_ENTITY_H