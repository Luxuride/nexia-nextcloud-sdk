//! Multistatus bodies used by the unit tests.
//!
//! Shared from one place so the request tests and the decoding tests agree
//! on exactly which answer they exercise.

/// One collection plus one file, shaped like a real Nextcloud answer.
pub(crate) const TWO_ENTRIES: &str = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:s="http://sabredav.org/ns" xmlns:oc="http://owncloud.org/ns" xmlns:nc="http://nextcloud.org/ns">
 <d:response>
  <d:href>/remote.php/dav/files/alice/Photos</d:href>
  <d:propstat>
   <d:prop>
<d:resourcetype><d:collection/></d:resourcetype>
<d:getetag>"5f1c0"</d:getetag>
<oc:fileid>9</oc:fileid>
<oc:permissions>RGDNVCK</oc:permissions>
<nc:has-preview>false</nc:has-preview>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
 <d:response>
  <d:href>/remote.php/dav/files/alice/Q1%20%26%20Q2%20%3Cfinal%3E.txt</d:href>
  <d:propstat>
   <d:prop>
<d:resourcetype/>
<d:getetag>"a1b2c3"</d:getetag>
<d:getcontentlength>128</d:getcontentlength>
<d:getcontenttype>text/plain; charset=utf-8</d:getcontenttype>
<d:getlastmodified>Wed, 21 Oct 2015 07:28:00 GMT</d:getlastmodified>
<oc:fileid>17</oc:fileid>
<oc:permissions>RDNVW</oc:permissions>
<nc:has-preview>true</nc:has-preview>
   </d:prop>
   <d:status>HTTP/1.1 200 OK</d:status>
  </d:propstat>
 </d:response>
</d:multistatus>"#;
