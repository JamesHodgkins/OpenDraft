#ifndef OD_SY_CIRCLE_H
#define OD_SY_CIRCLE_H

/**************************************************************************************
* OpenDraft:    System Circle Entity Class                                            *
*-------------------------------------------------------------------------------------*
* Filename:     OdCircle.h                                                            *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Circle Entity class for OpenDraft framework.                                      *
***************************************************************************************/


#include "System/Entities/OdEntity.h"


using namespace OD::System;

namespace OD
{
	namespace Geometry
	{

		class OdCircle : public OdEntity {
		public:
			OdVector2 center;
			double radius;

		};
	}
}


#endif // OD_SY_CIRCLE_H