#ifndef OD_DAT_DRAWINGDB_H
#define OD_DAT_DRAWINGDB_H

/**************************************************************************************
* OpenDraft:    Drawing Database Class                                                *
*-------------------------------------------------------------------------------------*
* Filename:     OdDrawingDb.h                                                         *
* Contributors: James Hodgkins                                                        *
* Date:         June 28, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Document class for OpenDraft framework.                                           *
***************************************************************************************/


#include <vector>
#include "System/Entities/OdEntity.h"


using namespace OD::Geometry;


namespace OD
{
	namespace Data
	{

		class OdDrawingDb
		{

			// Document Entities
			std::vector<OdEntity*> entities;	// Document Entities

		};

	} // namespace OD::Data
} // namespace OD


#endif // OD_DAT_DRAWINGDB_H
