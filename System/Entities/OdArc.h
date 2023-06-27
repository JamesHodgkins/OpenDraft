#ifndef OD_SY_ARC_H
#define OD_SY_ARC_H

/**************************************************************************************
* OpenDraft:    System Arc Entity Class                                               *
*-------------------------------------------------------------------------------------*
* Filename:     OdArc.h                                                               *
* Contributors: James Hodgkins                                                        *
* Date:         June 27, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Arc Entity class for OpenDraft framework.                                         *
***************************************************************************************/


#include "System/Entities/OdEntity.h"


using namespace OD::System;

namespace OD
{
	namespace Geometry
	{

		class OdArc : public OdEntity {
		public:

			OdVector2 center;
			double radius;
			double startAngle;
			double endAngle;

		};
	}
}


#endif // OD_SY_ARC_H