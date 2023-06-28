#ifndef OD_DAT_DOCUMENT_H
#define OD_DAT_DOCUMENT_H

/**************************************************************************************
* OpenDraft:    Document Class                                                        *
*-------------------------------------------------------------------------------------*
* Filename:     OdDocument.h                                                          *
* Contributors: James Hodgkins                                                        *
* Date:         June 28, 2023                                                         *
* Copyright:    ©2023 OpenDraft. All Rights Reserved.                                 *
*-------------------------------------------------------------------------------------*
* Description:                                                                        *
*   Document class for OpenDraft framework.                                           *
***************************************************************************************/



#include <string>
#include "DataManager/OdDrawingDb.h"
#include "System/OdCore.h"				// Include Core Utilities


namespace OD
{
	namespace Data
	{
		// Document Class
		class OdDocument
		{
		public:

			std::string name;			// Document Name
			std::string path;			// Document Path
			short version;				// Document Version
			bool readOnly;				// Document Read-Only
			bool modified;				// Document Modified
			bool locked;				// Document Locked

			OdDrawingDb database;		// Document Database


			// Constructors and Destructors
			OdDocument();
			~OdDocument();

			
			// 
			// Getters and Setters
			//
			
			// Document Name
			std::string getName();
			void setName(std::string aName);

			// Document Path
			std::string getPath();
			void setPath(std::string aPath);
			
			// Document Version
			short getVersion();
			void setVersion(short aVersion);

			// Document Read-Only
			bool isReadOnly();
			bool upgradeOpen();

			// Document Modified
			bool isModified();
			void setModified(bool aModified);

			// Document Locked
			bool isLocked();
			void setLocked(bool aLocked);

			// Document Save
			bool save();

			// Document Save As
			bool saveAs(std::string aPath);

		};

	}// namespace OD::Data
} // namespace OD

#endif // OD_SY_DOCUMENT_H
